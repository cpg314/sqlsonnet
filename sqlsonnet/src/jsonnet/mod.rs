mod formatter;
mod resolver;
pub use formatter::Jsonnet;
pub use resolver::{FsResolver, ImportResolver};

use std::path::PathBuf;

use jrsonnet_evaluator::parser::SourcePath;
use jrsonnet_gcmodule::Trace;
use jrsonnet_stdlib::StateExt;

use crate::error::JsonnetError;

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
    let state = jrsonnet_evaluator::State::default();
    state.with_stdlib();
    state.set_import_resolver(resolver);

    let val = evaluate_snippet("input.jsonnet", jsonnet, &state)?;
    let format = Box::new(jrsonnet_evaluator::manifest::JsonFormat::cli(3));
    val.manifest(format)
        .map_err(|e| JsonnetError::from(jsonnet, e))
}
