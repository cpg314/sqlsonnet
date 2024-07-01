mod cache;
mod error;
use error::*;
mod playground;
mod tracing_layer;

use std::collections::BTreeMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Context;
use axum::response::IntoResponse;
use clap::Parser;
use itertools::Itertools;
use metrics_exporter_prometheus::PrometheusBuilder;
use serde::{Deserialize, Serialize};
use tracing::*;

use sqlsonnet::{FsResolver, Queries, Query};

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
    /// Prepended to all requests
    #[clap(long)]
    prelude: Option<PathBuf>,
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

fn decode_query(
    request: String,
    state: State,
    compact: bool,
    limit: Option<usize>,
) -> Result<String, Error> {
    let request = [state.prelude()?, request].join("\n");
    let resolver = state.resolver;
    // We could also use OneOrMany from serde_with, but this seems to break error reporting.
    let mut query = {
        match Query::from_jsonnet(&request, resolver.clone()) {
            Ok(r) => Ok(r),
            Err(e) => {
                if let Ok(queries) = Queries::from_jsonnet(&request, resolver) {
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
    if let Some(limit) = limit {
        match &mut query {
            Query::Select(query) => query.limit = Some(limit),
        }
    }
    // Submit to Clickhouse and forward reply
    Ok::<String, Error>(query.to_sql(compact))
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
        let state = state.clone();
        tokio::task::spawn_blocking(move || decode_query(request, state, true, None)).await??
    };
    state
        .send_query(ClickhouseQuery { query: sql, params })
        .await
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

#[derive(Clone)]
struct State {
    client: reqwest::Client,
    args: Arc<Flags>,
    resolver: Arc<FsResolver>,
    cache: Option<Arc<cache::Cache>>,
}

impl State {
    fn new(args: &Flags) -> Result<Self, Error> {
        Ok(Self {
            client: reqwest::Client::new(),
            resolver: sqlsonnet::FsResolver::new(
                args.library.clone().map(|p| vec![p]).unwrap_or_default(),
            )
            .into(),
            cache: if let Some(path) = &args.cache {
                Some(Arc::new(cache::Cache::init(path)?))
            } else {
                None
            },
            args: Arc::new(args.clone()),
        })
    }

    fn prelude(&self) -> Result<String, Error> {
        Ok(format!(
            "{}\n{}",
            sqlsonnet::import("u", sqlsonnet::UTILS_FILENAME),
            self.args
                .prelude
                .as_ref()
                .map(|p| std::fs::read_to_string(p).map_err(|e| Error::Prelude(p.into(), e)))
                .transpose()?
                .unwrap_or_default()
        ))
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
    let handle = builder
        .install_recorder()
        .context("Failed to setup Prometheus metrics")?;

    if let Some(library) = &args.library {
        std::fs::create_dir_all(library).context("Failed to create library")?;
    }
    if let Some(prelude) = &args.prelude {
        anyhow::ensure!(prelude.is_file(), "Prelude {:?} not found", prelude);
    }

    info!("Testing connection with Clickhouse");
    let state = State::new(&args)?;
    state.test_clickhouse().await?;
    info!("Connected with Clickhouse");

    let app = tracing_layer::add_layer(
        axum::Router::new()
            .route("/", axum::routing::post(handle_query))
            .nest("/play", playground::router())
            .route(
                "/metrics",
                axum::routing::get(|| async move { handle.render() }),
            )
            .with_state(state),
    );

    let address = format!("0.0.0.0:{}", args.port);
    info!("Serving on {}", address);
    let listener = tokio::net::TcpListener::bind(address).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
