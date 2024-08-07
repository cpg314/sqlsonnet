//! Interpretation of Jsonnet code.
mod formatter;
mod resolver;
pub use formatter::Jsonnet;
pub use resolver::{FsResolver, ImportResolver};

use std::path::PathBuf;

use jrsonnet_evaluator::{parser::SourcePath, trace::PathResolver};
use jrsonnet_gcmodule::Trace;

use crate::error::JsonnetError;

/// Filename for the embedded utilities library.
pub const UTILS_FILENAME: &str = "sqlsonnet.libsonnet";
/// Name of the `extVar` where the user agent is stored.
pub const AGENT_VAR: &str = "sqlsonnet-user-agent";

/// Import instruction for the embedded utilities
pub fn import_utils() -> String {
    import("u", UTILS_FILENAME)
}
/// Jsonnet import statement `local {variable} = import '{filename}';`
pub fn import(variable: &str, filename: &str) -> String {
    format!("local {} = import '{}';", variable, filename)
}

fn evaluate_snippet(
    filename: &str,
    src: &str,
    state: &jrsonnet_evaluator::State,
) -> Result<jrsonnet_evaluator::Val, crate::error::JsonnetError> {
    state
        .evaluate_snippet(filename, src)
        .map_err(|e| JsonnetError::from(src, e))
}

/// Options for jsonnet interpretation.
pub struct Options<'a, R: ImportResolver> {
    /// Import resolver
    pub resolver: R,
    /// User agent, stored in [`AGENT_VAR`]
    pub agent: &'a str,
}

impl Default for Options<'static, FsResolver> {
    fn default() -> Self {
        Self {
            resolver: FsResolver::default(),
            agent: "",
        }
    }
}
impl<R: ImportResolver> From<R> for Options<'static, R> {
    fn from(resolver: R) -> Self {
        Self {
            resolver,
            agent: "",
        }
    }
}

/// Evaluate Jsonnet into JSON
pub fn evaluate<R: ImportResolver>(
    jsonnet: &str,
    options: Options<R>,
) -> Result<String, crate::error::JsonnetError> {
    let mut state = jrsonnet_evaluator::StateBuilder::default();
    state.import_resolver(options.resolver.to_resolver());

    let context = jrsonnet_stdlib::ContextInitializer::new(PathResolver::new_cwd_fallback());
    // We should always set this, as it is not possible to know if an extVar is defined at runtime
    context.add_ext_str(AGENT_VAR.into(), options.agent.into());
    state.context_initializer(context);

    let state = state.build();

    let val = evaluate_snippet("input.jsonnet", jsonnet, &state)?;
    let format = Box::new(jrsonnet_evaluator::manifest::JsonFormat::cli(3));
    val.manifest(format)
        .map_err(|e| JsonnetError::from(jsonnet, e))
}
