mod error;
pub use error::Error;
mod jsonnet;
mod queries;
mod sql_parse;
mod to_sql;
pub use jsonnet::{ImportPaths, Jsonnet};
pub use queries::{Queries, Query};

macro_rules! impl_conversions {
    ($t: ty, $rule: path) => {
        impl $t {
            pub fn as_jsonnet(&self) -> Jsonnet {
                serde_json::to_value(self).unwrap().into()
            }
            pub fn from_sql(input: &str) -> Result<Self, Error> {
                Ok(sql_parse::query_from_sql(input, $rule)?)
            }
            pub fn from_jsonnet(input: &str, import_paths: ImportPaths) -> Result<Self, Error> {
                let json = jsonnet::evaluate(input, import_paths)?;
                Ok(serde_json::from_str(&json)
                    .map_err(|e| crate::error::JsonError::from(&json, e))?)
            }
            pub fn to_sql(&self, compact: bool) -> String {
                to_sql::ToSql::to_sql_str(self, compact)
            }
        }
    };
}

impl_conversions!(Queries, sql_parse::Rule::queries);
impl_conversions!(Query, sql_parse::Rule::query);

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
