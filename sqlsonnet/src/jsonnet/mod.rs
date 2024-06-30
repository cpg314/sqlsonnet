mod formatter;
mod resolver;
pub use formatter::Jsonnet;
pub use resolver::ImportPaths;

use std::path::{Path, PathBuf};

use itertools::Itertools;
use jrsonnet_evaluator::parser::SourcePath;
use jrsonnet_gcmodule::Trace;
use jrsonnet_stdlib::StateExt;

use crate::error::JsonnetError;

const UTILS_FILENAME: &str = "sqlsonnet.libsonnet";

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
    import_paths: ImportPaths,
) -> Result<String, crate::error::JsonnetError> {
    let state = jrsonnet_evaluator::State::default();
    state.with_stdlib();
    state.set_import_resolver(resolver::Resolver::new(import_paths));

    let val = evaluate_snippet("input.jsonnet", jsonnet, &state)?;
    let format = Box::new(jrsonnet_evaluator::manifest::JsonFormat::cli(3));
    val.manifest(format)
        .map_err(|e| JsonnetError::from(jsonnet, e))
}
