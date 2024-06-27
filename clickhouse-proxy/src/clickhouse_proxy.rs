use axum::{http::Request, response::Response};
use clap::Parser;
use itertools::Itertools;
use sqlsonnet::{ImportPaths, Query};
use std::collections::BTreeMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tracing::*;

mod cache;
mod error;
use error::*;

/// Reverse proxies a Clickhouse HTTP server, transforming Jsonnet or JSON queries into SQL.
/// WARN: For now, the server assumes that the client are trusted. For example, they might be able
///       to access the filesystem via standard library calls, cause large resource usage, etc.
#[derive(Clone, Parser)]
#[clap(version)]
struct Flags {
    // URL to the Clickhouse HTTP endpoint
    #[clap(long, env = "CLICKHOUSE_URL")]
    url: reqwest::Url,
    /// Clickhouse username
    #[clap(long, env = "CLICKHOUSE_USERNAME")]
    username: String,
    #[clap(long, env = "CLICKHOUSE_PASSWORD")]
    password: Option<String>,
    #[clap(long)]
    cache: Option<PathBuf>,
    /// Folder with Jsonnet library files
    #[clap(long)]
    library: Option<PathBuf>,
    #[clap(long)]
    port: u16,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    if let Err(e) = main_impl().await {
        error!("{:?}", e);
        std::process::exit(1);
    }
    Ok(())
}

#[derive(Hash, Eq, PartialEq)]
struct SqlQuery(String);
impl From<String> for SqlQuery {
    fn from(source: String) -> Self {
        Self(source)
    }
}
impl From<&str> for SqlQuery {
    fn from(source: &str) -> Self {
        Self(source.into())
    }
}

async fn handle_query(
    axum::extract::Query(params): axum::extract::Query<BTreeMap<String, String>>,
    axum::extract::State(state): axum::extract::State<State>,
    request: String,
) -> Result<axum::http::Response<reqwest::Body>, Error> {
    // Remove whitespace and comments for logging
    let request_log = request
        .lines()
        .filter(|l| !l.trim_start().starts_with("//") && !l.is_empty())
        .flat_map(|l| l.split(' '))
        .filter(|l| !l.is_empty())
        .join(" ");
    info!(request = request_log, "Handling query");
    let sql = if request.starts_with("SELECT") {
        request
    } else {
        // Compile
        let library: ImportPaths = state
            .args
            .library
            .as_ref()
            .map(|l| l.into())
            .unwrap_or_default();
        tokio::task::spawn_blocking(move || {
            // Automatically add the imports
            let request = [library.imports(), request].join("\n");
            let query = Query::from_jsonnet(&request, library)?;
            info!("Compiled query");
            // Submit to Clickhouse and forward reply
            Ok::<String, Error>(query.to_sql(true))
        })
        .await??
    };
    info!(sql, "Sending query to Clickhouse");
    let resp = state.send_query(sql.into(), params).await?;
    Ok(resp.into())
}

#[derive(Clone)]
struct State {
    client: reqwest::Client,
    args: Arc<Flags>,
    _cache: Option<Arc<cache::Cache>>,
}

impl State {
    fn new(args: &Flags) -> Result<Self, Error> {
        Ok(Self {
            client: reqwest::Client::new(),
            _cache: if let Some(path) = &args.cache {
                Some(Arc::new(cache::Cache::init(path)?))
            } else {
                None
            },
            args: Arc::new(args.clone()),
        })
    }
    async fn send_query(
        &self,
        query: SqlQuery,
        params: BTreeMap<String, String>,
    ) -> Result<reqwest::Response, ClickhouseError> {
        let mut hasher = DefaultHasher::default();
        query.hash(&mut hasher);
        params.hash(&mut hasher);
        let id = hasher.finish();
        info!(id, "Sending query to clickhouse");
        Ok(self
            .client
            .post(self.args.url.clone())
            .body(query.0)
            .query(&params)
            .header(reqwest::header::TRANSFER_ENCODING, "chunked")
            .basic_auth(&self.args.username, self.args.password.clone())
            .send()
            .await?
            .error_for_status()?)
    }
    async fn test_clickhouse(&self) -> Result<(), ClickhouseError> {
        let resp = self
            .send_query("SELECT 1+1".into(), Default::default())
            .await?;
        let headers = resp.headers().clone();
        let resp = resp.text().await?;
        if resp.trim() != "2" {
            return Err(ClickhouseError::Connect(resp.into(), headers));
        }
        Ok(())
    }
}

async fn main_impl() -> anyhow::Result<()> {
    let args = Flags::parse();
    sqlsonnet::setup_logging();

    info!("Testing connection with Clickhouse");
    let state = State::new(&args)?;
    state.test_clickhouse().await?;
    info!("Connected with Clickhouse");

    let app = axum::Router::new()
        .route("/", axum::routing::post(handle_query))
        .with_state(state)
        .layer(
            tower_http::trace::TraceLayer::new_for_http()
                .make_span_with(|request: &axum::http::Request<_>| {
let id = uuid::Uuid::new_v4();
                    let matched_path = request
                        .extensions()
                        .get::<axum::extract::MatchedPath>()
                        .map(axum::extract::MatchedPath::as_str);
                    info_span!(
                        "request",
                        uuid= id.to_string(),
                        method = ?request.method(),
                        agent = request.headers().get(axum::http::header::USER_AGENT).and_then(|v| v.to_str().ok() ).unwrap_or_default(),
                        matched_path,
                        some_other_field = tracing::field::Empty,
                    )
                })
                .on_request(|_request: &Request<_>, _span: &Span| {
                    info!("Serving request");
                })
                .on_response(|response: &Response, latency: Duration, _span: &Span| {
                    let code =response.status();
                    if code.is_client_error() {
                        warn!( ?latency, ?code, "Sending response with client error");
                    } else if code.is_server_error() {
                        error!( ?latency, ?code, "Sending response with server error");
                    } else {
                        info!( ?latency, ?code, "Sending response");
                    };
                })
                .on_body_chunk(|_chunk: &axum::body::Bytes, _latency: Duration, _span: &Span| {})
                .on_eos(
                    |_trailers: Option<&axum::http::HeaderMap>,
                     _stream_duration: Duration,
                     _span: &Span| {},
                )
                .on_failure(
                    |error: tower_http::classify::ServerErrorsFailureClass,
                     latency: Duration,
                     _span: &Span| {
                        warn!(?error, ?latency, "Encountered error");
                    },
                ),
        );

    let address = format!("0.0.0.0:{}", args.port);
    info!("Serving on {}", address);
    let listener = tokio::net::TcpListener::bind(address).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
