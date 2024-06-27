use axum::response::Response;
use tracing::*;

use super::cache::CacheError;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Sqlsonnet error: {0}")]
    SqlSonnet(#[from] sqlsonnet::Error),
    #[error("Multiple queries are not supported")]
    MultipleQueries,
    #[error("Clickhouse error: {0}")]
    Clickhouse(#[from] ClickhouseError),
    #[error("Cache error: {0}")]
    Cache(#[from] CacheError),
}
#[derive(thiserror::Error, Debug)]
pub enum ClickhouseError {
    #[error("Connection error {0} {1:?}")]
    Connect(String, reqwest::header::HeaderMap),
    #[error("Query execution failure: {0}")]
    QueryFailure(#[from] reqwest::Error),
}
impl axum::response::IntoResponse for Error {
    fn into_response(self) -> Response {
        let code = match self {
            Error::SqlSonnet(_) | Error::MultipleQueries => axum::http::StatusCode::BAD_REQUEST,
            Error::Clickhouse(_) => axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Error::Cache(_) => todo!(),
        };
        (code, self.to_string()).into_response()
    }
}
