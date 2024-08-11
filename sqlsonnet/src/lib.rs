#![doc = include_str!("../README.md")]

//! ## Usage from Rust
//!
//! ```
//! use sqlsonnet::{Query, sqlsonnet, jsonnet::Options};
//! // This performs compile-time syntax checking
//! let jsonnet: &str = sqlsonnet!({ select: { fields: ["name", "age"], from: "contacts" } });
//! // Parse jsonnet
//! let query = Query::from_jsonnet(jsonnet, Options::default()).unwrap();
//! // Convert to SQL
//! assert_eq!(query.to_sql(true), "SELECT name, age FROM contacts");
//! ```

mod error;
pub use error::{Error, FormattedError};
#[cfg(feature = "from-sql")]
mod from_sql;
pub mod jsonnet;
pub mod queries;
mod to_sql;
pub use jsonnet::Jsonnet;
pub use queries::{Queries, Query};

pub use sqlsonnet_macros::sqlsonnet;

macro_rules! impl_conversions {
    ($t: ty, $rule: path) => {
        impl $t {
            /// Convert to Jsonnet.
            pub fn as_jsonnet(&self) -> Jsonnet {
                serde_json::to_value(self).unwrap().into()
            }
            /// Convert from SQL.
            #[cfg(feature = "from-sql")]
            pub fn from_sql(input: &str) -> Result<Self, Error> {
                Ok(from_sql::query_from_sql(input, $rule)?)
            }
            /// Convert from Jsonnet.
            pub fn from_jsonnet<R: jsonnet::ImportResolver>(
                input: &str,
                options: jsonnet::Options<R>,
            ) -> Result<Self, Error> {
                let json = jsonnet::evaluate(input, options)?;
                Ok(serde_json::from_str(&json)
                    .map_err(|e| crate::error::JsonError::from(&json, e))?)
            }
            /// Convert to SQL.
            pub fn to_sql(&self, compact: bool) -> String {
                to_sql::ToSql::to_sql_str(self, compact)
            }
        }
    };
}

impl_conversions!(Queries, from_sql::Rule::queries);
impl_conversions!(Query, from_sql::Rule::query);

/// Setup logger on stderr.
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
