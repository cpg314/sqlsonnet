mod error;
pub use error::{Error, FormattedError};
mod from_sql;
mod jsonnet;
pub mod queries;
mod to_sql;
pub use jsonnet::{import_utils, FsResolver, ImportResolver, Jsonnet, Options as JsonnetOptions};
pub use queries::{Queries, Query};

macro_rules! impl_conversions {
    ($t: ty, $rule: path) => {
        impl $t {
            pub fn as_jsonnet(&self) -> Jsonnet {
                serde_json::to_value(self).unwrap().into()
            }
            pub fn from_sql(input: &str) -> Result<Self, Error> {
                Ok(from_sql::query_from_sql(input, $rule)?)
            }
            pub fn from_jsonnet<'a, R: ImportResolver>(
                input: &str,
                options: impl Into<JsonnetOptions<'a, R>>,
            ) -> Result<Self, Error> {
                let json = jsonnet::evaluate(input, options.into())?;
                Ok(serde_json::from_str(&json)
                    .map_err(|e| crate::error::JsonError::from(&json, e))?)
            }
            pub fn to_sql(&self, compact: bool) -> String {
                to_sql::ToSql::to_sql_str(self, compact)
            }
        }
    };
}

impl_conversions!(Queries, from_sql::Rule::queries);
impl_conversions!(Query, from_sql::Rule::query);

pub fn setup_logging() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::builder()
                .with_default_directive(tracing_subscriber::filter::LevelFilter::INFO.into())
                .from_env_lossy(),
        )
        .with_writer(std::io::stderr)
        .init()
}
