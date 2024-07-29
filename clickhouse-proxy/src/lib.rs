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
use tracing::*;

use clickhouse_client::ClickhouseQuery;
use sqlsonnet::{FsResolver, Queries, Query};

lazy_static::lazy_static! {
    pub static ref VARIABLE_RE: regex::Regex = regex::Regex::new(r#"\$\{(.*?)\}"#).unwrap();
}

/// Reverse proxies a Clickhouse HTTP server, transforming Jsonnet or JSON queries into SQL.
///
/// WARN: For now, the server assumes that the client are trusted. For example, they might be able
///       to access the filesystem via standard library calls, cause large resource usage, etc.
#[derive(Clone, Parser)]
#[clap(version)]
pub struct Flags {
    // URL to the Clickhouse HTTP endpoint, with username and password if necessary
    #[clap(long, env = "CLICKHOUSE_URL")]
    pub url: reqwest::Url,
    #[clap(long)]
    pub cache: Option<PathBuf>,
    /// Folder with Jsonnet library files
    #[clap(long)]
    pub library: Option<PathBuf>,
    /// Folder with shared snippets
    #[clap(long)]
    pub shares: Option<PathBuf>,
    /// Prepended to all requests
    #[clap(long)]
    pub prelude: Option<PathBuf>,
    #[clap(long)]
    pub port: u16,
}

fn decode_query(
    request: &str,
    state: State,
    compact: bool,
    limit: Option<usize>,
) -> Result<String, Error> {
    let resolver = state.resolver;

    let queries = Queries::from_jsonnet(request, resolver)?;
    let mut query = if queries.len() == 1 {
        queries.into_iter().next().unwrap()
    } else {
        return Err(Error::MultipleQueries(queries.len()));
    };
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
    headers: axum::http::HeaderMap,
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
    let sql = if request.to_lowercase().starts_with("select") {
        request
    } else {
        let state = state.clone();
        let request = [state.prelude()?, request].join("\n");
        tokio::task::spawn_blocking(move || decode_query(&request, state, true, None)).await??
    };
    state
        .send_query(ClickhouseQuery {
            query: sql,
            params,
            compression: clickhouse_client::Compression::from_headers(&headers),
        })
        .await
}

pub struct PreparedRequest {
    id: u64,
    query: Option<ClickhouseQuery>,
    builder: clickhouse_client::PreparedRequest,
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
    pub async fn send(self) -> Result<reqwest::Response, clickhouse_client::Error> {
        info!("Sending query to Clickhouse");
        self.builder.send().await
    }
}

#[derive(Clone)]
struct State {
    client: clickhouse_client::HttpClient,
    args: Arc<Flags>,
    resolver: Arc<FsResolver>,
    cache: Option<Arc<cache::Cache>>,
}

impl State {
    fn new(args: &Flags) -> Result<Self, Error> {
        Ok(Self {
            // We set the compression to `false` to not decompress the body
            client: clickhouse_client::HttpClient::new(args.url.clone(), false),
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
            sqlsonnet::import_utils(),
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
        let builder = self.client.prepare_request(&query);
        PreparedRequest {
            id: hasher.finish(),
            query: Some(query),
            builder,
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
    async fn test_clickhouse(&self) -> Result<(), Error> {
        let resp = self
            .client
            .send_query(&ClickhouseQuery {
                query: "SELECT 1+1".into(),
                params: Default::default(),
                compression: clickhouse_client::Compression::None,
            })
            .await?
            .text()
            .await
            .map_err(clickhouse_client::Error::from)?;
        if resp.trim() != "2" {
            return Err(Error::ClickhousePing(resp));
        }
        Ok(())
    }
}

pub async fn main_impl(args: Flags) -> anyhow::Result<()> {
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
