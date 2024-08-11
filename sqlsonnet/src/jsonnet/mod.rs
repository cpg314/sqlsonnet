//! Interpretation of Jsonnet code.
mod formatter;
mod resolver;
pub use formatter::Jsonnet;
pub use resolver::{FsResolver, ImportResolver};

use std::collections::HashMap;
use std::path::PathBuf;

pub use jrsonnet_evaluator::{parser::SourcePath, trace::PathResolver, val::NumValue, Val};
pub use jrsonnet_gcmodule;
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
pub struct Options<R: ImportResolver> {
    /// Import resolver
    pub resolver: R,
    ext_vars: HashMap<String, Val>,
}
impl Default for Options<FsResolver> {
    fn default() -> Self {
        Self {
            resolver: FsResolver::default(),
            ext_vars: Default::default(),
        }
    }
}

pub trait Value {
    fn try_into_val(self) -> Result<Val, crate::Error>;
}
impl Value for &str {
    fn try_into_val(self) -> Result<Val, crate::Error> {
        Ok(Val::from(self))
    }
}
macro_rules! val {
    ($t: ty) => {
        impl Value for $t {
            fn try_into_val(self) -> Result<Val, crate::Error> {
                Ok(Val::Num(NumValue::try_from(self)?))
            }
        }
    };
}
macro_rules! val_infaillible {
    ($t: ty) => {
        impl Value for $t {
            fn try_into_val(self) -> Result<Val, crate::Error> {
                Ok(Val::Num(NumValue::try_from(self).unwrap()))
            }
        }
    };
}
val!(f32);
val!(f64);
val_infaillible!(u16);
val_infaillible!(i16);
val_infaillible!(u8);
val_infaillible!(i8);
val!(u64);
val!(i64);
val_infaillible!(u32);
val_infaillible!(i32);

impl<R: ImportResolver> Options<R> {
    pub fn add_var(&mut self, name: &str, var: impl Value) -> Result<(), crate::Error> {
        self.ext_vars.insert(name.into(), var.try_into_val()?);
        Ok(())
    }
    pub fn new(resolver: R, agent: &str) -> Self {
        Self {
            resolver,
            ext_vars: HashMap::from([(AGENT_VAR.into(), agent.into())]),
        }
    }
}

/// Evaluate Jsonnet into JSON
pub fn evaluate<R: ImportResolver>(
    jsonnet: &str,
    mut options: Options<R>,
) -> Result<String, crate::error::JsonnetError> {
    let mut state = jrsonnet_evaluator::StateBuilder::default();
    state.import_resolver(options.resolver.to_resolver());

    let context = jrsonnet_stdlib::ContextInitializer::new(PathResolver::new_cwd_fallback());
    // Make sure AGENT_VAR is always set as it is not possible to know if an extVar is defined at runtime
    options.ext_vars.entry(AGENT_VAR.into()).or_default();
    // Add variables
    for (k, v) in options.ext_vars {
        context.add_ext_var(k.into(), v);
    }

    state.context_initializer(context);

    let state = state.build();

    let val = evaluate_snippet("input.jsonnet", jsonnet, &state)?;
    let format = Box::new(jrsonnet_evaluator::manifest::JsonFormat::cli(3));
    val.manifest(format)
        .map_err(|e| JsonnetError::from(jsonnet, e))
}
