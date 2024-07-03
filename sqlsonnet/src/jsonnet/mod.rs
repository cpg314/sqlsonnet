mod formatter;
mod resolver;
pub use formatter::Jsonnet;
pub use resolver::{FsResolver, ImportResolver};

use std::path::PathBuf;

use jrsonnet_evaluator::{parser::SourcePath, trace::PathResolver};
use jrsonnet_gcmodule::Trace;

use crate::error::JsonnetError;

const UTILS_FILENAME: &str = "sqlsonnet.libsonnet";

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

/// Evaluate Jsonnet into JSON
pub fn evaluate(
    jsonnet: &str,
    resolver: impl jrsonnet_evaluator::ImportResolver,
) -> Result<String, crate::error::JsonnetError> {
    let mut state = jrsonnet_evaluator::StateBuilder::default();
    state.import_resolver(resolver);
    state.context_initializer(jrsonnet_stdlib::ContextInitializer::new(
        PathResolver::new_cwd_fallback(),
    ));
    let state = state.build();

    let val = evaluate_snippet("input.jsonnet", jsonnet, &state)?;
    let format = Box::new(jrsonnet_evaluator::manifest::JsonFormat::cli(3));
    val.manifest(format)
        .map_err(|e| JsonnetError::from(jsonnet, e))
}
