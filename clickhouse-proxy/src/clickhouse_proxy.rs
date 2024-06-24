use axum::{http::Request, response::Response};
use clap::Parser;
use itertools::Itertools;
use sqlsonnet::Queries;
use std::time::Duration;
use tracing::*;

/// Reverse proxies a Clickhouse HTTP server, transforming Jsonnet or JSON queries into SQL.
#[derive(Parser)]
#[clap(version)]
struct Flags {
    /// URL to the Clickhouse HTTP endpoint
    // #[clap(long, env = "CLICKHOUSE_URL")]
    // url: reqwest::Url,
    // /// Clickhouse username
    // #[clap(long, env = "CLICKHOUSE_USERNAME")]
    // username: String,
    #[clap(long, env = "CLICKHOUSE_PASSWORD")]
    password: Option<String>,
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

#[derive(thiserror::Error, Debug)]
enum Error {
    #[error("Sqlsonnet error")]
    SqlSonnet(#[from] sqlsonnet::Error),
    #[error("Multiple queries are not supported")]
    MultipleQueries,
}
impl axum::response::IntoResponse for Error {
    fn into_response(self) -> Response {
        let code = axum::http::StatusCode::BAD_REQUEST;
        (code, self.to_string()).into_response()
    }
}

async fn handle_query(
    axum::extract::State(_state): axum::extract::State<State>,
    request: String,
) -> Result<(), Error> {
    let request = request
        .replace('\n', " ")
        .trim()
        .split(' ')
        .filter(|x| !x.is_empty())
        .join(" ");
    info!(request, "Handling query");
    let queries = Queries::from_jsonnet(&request)?;
    info!(queries = queries.len(), "Compiled queries");
    if queries.len() > 1 {
        return Err(Error::MultipleQueries);
    }
    let sql = queries.to_sql(true);
    info!(sql, "Sending queries to Clickhouse");
    Ok(())
}

#[derive(Clone)]
struct State {
    _client: reqwest::Client,
}
impl State {
    fn new() -> Self {
        Self {
            _client: reqwest::Client::new(),
        }
    }
}

async fn main_impl() -> anyhow::Result<()> {
    let args = Flags::parse();
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        .init();

    let app = axum::Router::new()
        .route("/", axum::routing::post(handle_query))
        .with_state(State::new())
        .layer(
            tower_http::trace::TraceLayer::new_for_http()
                .make_span_with(|request: &axum::http::Request<_>| {
                    let matched_path = request
                        .extensions()
                        .get::<axum::extract::MatchedPath>()
                        .map(axum::extract::MatchedPath::as_str);
                    info_span!(
                        "request",
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

    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", args.port)).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
