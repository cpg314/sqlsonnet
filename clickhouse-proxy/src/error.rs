use std::path::PathBuf;

use axum::response::Response;
use tracing::*;

use super::cache::CacheError;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Sqlsonnet error: {0}")]
    SqlSonnet(#[from] sqlsonnet::Error),
    #[error("Clickhouse error: {0}")]
    Clickhouse(#[from] ClickhouseError),
    #[error("Cache error: {0}")]
    Cache(#[from] CacheError),
    #[error("Task panicked: {0}")]
    Join(#[from] tokio::task::JoinError),
    #[error("Exactly one query must be provided (received {0} queries)")]
    MultipleQueries(usize),
    #[error("Could not convert body to bytes: {0}")]
    ConvertBody(axum::Error),
    #[error("Failed to read prelude {0}: {1}")]
    Prelude(PathBuf, std::io::Error),
    #[error(transparent)]
    Sharing(#[from] SharingError),
    #[error(transparent)]
    Websocket(#[from] WebsocketError),
    #[error("jsonnetfmt error: {0}")]
    JsonnetFmt(String),
}

#[derive(thiserror::Error, Debug)]
pub enum WebsocketError {
    #[error("Internal error: {0}")]
    Internal(#[from] axum::Error),
    #[error("Invalid message: {0}")]
    InvalidMessage(#[from] serde_json::Error),
}
#[derive(thiserror::Error, Debug)]
pub enum SharingError {
    #[error("Not enabled")]
    NotEnabled,
    #[error("IO error: {0}")]
    IO(#[from] std::io::Error),
    #[error("Invalid share ID")]
    InvalidId,
}

#[derive(thiserror::Error, Debug)]
pub enum ClickhouseError {
    #[error("Connection error {0} {1:?}")]
    Connect(String, reqwest::header::HeaderMap),
    #[error("Query execution failure: {0}")]
    QueryFailure(#[from] reqwest::Error),
}

impl From<Error> for sqlsonnet::FormattedError {
    fn from(value: Error) -> Self {
        match value {
            Error::SqlSonnet(e) => e.into(),
            _ => value.to_string().into(),
        }
    }
}

impl axum::response::IntoResponse for Error {
    fn into_response(self) -> Response {
        let code = match self {
            Error::SqlSonnet(_) | Error::MultipleQueries(_) => axum::http::StatusCode::BAD_REQUEST,
            _ => axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        };
        (code, self.to_string()).into_response()
    }
}

impl axum::response::IntoResponse for SharingError {
    fn into_response(self) -> Response {
        (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            self.to_string(),
        )
            .into_response()
    }
}
