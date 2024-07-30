mod formatter;
mod resolver;
pub use formatter::Jsonnet;
pub use resolver::{FsResolver, ImportResolver};

use std::path::PathBuf;

use jrsonnet_evaluator::{parser::SourcePath, trace::PathResolver};
use jrsonnet_gcmodule::Trace;

use crate::error::JsonnetError;

const UTILS_FILENAME: &str = "sqlsonnet.libsonnet";
const AGENT_VAR: &str = "sqlsonnet-user-agent";

pub fn import_utils() -> String {
    import("u", UTILS_FILENAME)
}
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

pub struct Options<'a, R: ImportResolver> {
    pub resolver: R,
    pub agent: &'a str,
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
