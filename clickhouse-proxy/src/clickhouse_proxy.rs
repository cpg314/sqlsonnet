mod cache;
mod error;
use error::*;

use std::collections::BTreeMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use axum::{
    http::Request,
    response::{IntoResponse, Response},
};
use clap::Parser;
use itertools::Itertools;
use metrics_exporter_prometheus::PrometheusBuilder;
use serde::{Deserialize, Serialize};
use tracing::*;

use sqlsonnet::{ImportPaths, Queries, Query};

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

fn decode_query(request: String, library: ImportPaths) -> Result<String, Error> {
    // Automatically add the imports
    let request = [library.imports(), request].join("\n");
    // We could also use OneOrMany from serde_with, but this seems to break error reporting.
    let query = {
        match Query::from_jsonnet(&request, library.clone()) {
            Ok(r) => Ok(r),
            Err(e) => {
                if let Ok(queries) = Queries::from_jsonnet(&request, library) {
                    if queries.len() == 1 {
                        Ok(queries.into_iter().next().unwrap())
                    } else {
                        Err(Error::MultipleQueries(queries.len()))
                    }
                } else {
                    Err(e.into())
                }
            }
        }
    }?;
    // Submit to Clickhouse and forward reply
    Ok::<String, Error>(query.to_sql(true))
}

async fn handle_query(
    axum::extract::Query(params): axum::extract::Query<BTreeMap<String, String>>,
    axum::extract::State(state): axum::extract::State<State>,
    request: String,
) -> Result<axum::response::Response, Error> {
    metrics::counter!("requests").increment(1);
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
        tokio::task::spawn_blocking(move || decode_query(request, library)).await??
    };
    state
        .send_query(ClickhouseQuery { query: sql, params })
        .await
}

#[derive(Clone)]
struct State {
    client: reqwest::Client,
    args: Arc<Flags>,
    cache: Option<Arc<cache::Cache>>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Hash)]
pub struct ClickhouseQuery {
    query: String,
    params: BTreeMap<String, String>,
}
impl ClickhouseQuery {
    fn heartbeat() -> Self {
        Self {
            query: "SELECT 1+1".into(),
            params: Default::default(),
        }
    }
}
pub struct PreparedRequest {
    id: u64,
    query: Option<ClickhouseQuery>,
    builder: reqwest::RequestBuilder,
}
impl std::fmt::Debug for PreparedRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.id)
    }
}
impl PreparedRequest {
    pub fn take_query(&mut self) -> Option<ClickhouseQuery> {
        self.query.take()
    }
    #[tracing::instrument()]
    pub async fn send(self) -> Result<reqwest::Response, ClickhouseError> {
        info!("Sending query to Clickhouse");
        self.builder.send().await.map_err(ClickhouseError::from)
    }
}

impl State {
    fn new(args: &Flags) -> Result<Self, Error> {
        Ok(Self {
            client: reqwest::Client::new(),
            cache: if let Some(path) = &args.cache {
                Some(Arc::new(cache::Cache::init(path)?))
            } else {
                None
            },
            args: Arc::new(args.clone()),
        })
    }
    fn prepare_request(&self, query: ClickhouseQuery) -> PreparedRequest {
        // Hash query
        let mut hasher = DefaultHasher::default();
        query.hash(&mut hasher);
        self.args.username.hash(&mut hasher);
        self.args.password.hash(&mut hasher);
        let inner = self
            .client
            .post(self.args.url.clone())
            .body(query.query.clone())
            .query(&query.params)
            .header(reqwest::header::TRANSFER_ENCODING, "chunked")
            .basic_auth(&self.args.username, self.args.password.clone());
        PreparedRequest {
            id: hasher.finish(),
            query: Some(query),
            builder: inner,
        }
    }
    async fn send_query(&self, query: ClickhouseQuery) -> Result<axum::response::Response, Error> {
        let request = self.prepare_request(query);

        if let Some(cache) = &self.cache {
            Ok(cache.process(request).await?)
        } else {
            Ok(request
                .send()
                .await
                // reqwest::Response to http::Response<reqwest::Body>
                .map(axum::http::Response::<reqwest::Body>::from)?
                .into_response())
        }
    }
    async fn test_clickhouse(&self) -> Result<(), ClickhouseError> {
        let resp = self
            .prepare_request(ClickhouseQuery::heartbeat())
            .send()
            .await?;
        let headers = resp.headers().clone();
        let resp = resp.text().await?;
        if resp.trim() != "2" {
            return Err(ClickhouseError::Connect(resp, headers));
        }
        Ok(())
    }
}

async fn main_impl() -> anyhow::Result<()> {
    let args = Flags::parse();
    sqlsonnet::setup_logging();

    let builder = PrometheusBuilder::new();
    let handle = builder.install_recorder()?;

    info!("Testing connection with Clickhouse");
    let state = State::new(&args)?;
    state.test_clickhouse().await?;
    info!("Connected with Clickhouse");

    let app = axum::Router::new()
        .route("/", axum::routing::post(handle_query))
        .route("/metrics", axum::routing::get(||async move {
            handle.render()
        }))
        .with_state(state)
        .layer(
            tower_http::trace::TraceLayer::new_for_http()
                .make_span_with(|request: &axum::http::Request<_>| {
let id = uuid::Uuid::new_v4();
                    let path = request
                        .extensions()
                        .get::<axum::extract::MatchedPath>()
                        .map(axum::extract::MatchedPath::as_str);
                    info_span!(
                        "request",
                        uuid= id.to_string(),
                        method = ?request.method(),
                        agent = request.headers().get(axum::http::header::USER_AGENT).and_then(|v| v.to_str().ok() ).unwrap_or_default(),
                        path,
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
