use std::path::{Path, PathBuf};

use itertools::Itertools;
use jrsonnet_evaluator::parser::SourcePath;
use jrsonnet_gcmodule::Trace;
use jrsonnet_stdlib::StateExt;
use serde::Serialize;

use crate::error::JsonnetError;

const UTILS_FILENAME: &str = "sqlsonnet.libsonnet";

/// Jsonnet code that implemens [`std::fmt::Display`]
pub struct Jsonnet(serde_json::Value);
impl From<serde_json::Value> for Jsonnet {
    fn from(source: serde_json::Value) -> Self {
        Self(source)
    }
}

impl std::fmt::Display for Jsonnet {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let out = vec![];
        let writer = std::io::BufWriter::new(out);
        let mut serializer =
            serde_json::ser::Serializer::with_formatter(writer, JsonnetFormatter::default());
        self.0.serialize(&mut serializer).unwrap();
        let out = serializer.into_inner().into_inner().unwrap();
        String::from_utf8(out).unwrap().fmt(f)
    }
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

#[derive(Default)]
pub struct ImportPaths(Vec<PathBuf>);
impl ImportPaths {
    // Produces statements of the form
    // local file_stem = import 'path.libsonnet';
    pub fn imports(&self) -> String {
        self.0
            .iter()
            .map(|f| glob::glob(f.join("*.libsonnet").to_str().unwrap()))
            .filter_map(|glob| glob.ok())
            .flatten()
            .filter_map(|f| f.ok())
            .map(|f| {
                format!(
                    "local {} = import '{}';",
                    f.file_stem().unwrap().to_string_lossy(),
                    f.file_name().unwrap().to_string_lossy()
                )
            })
            .chain(std::iter::once(format!(
                "local u = import '{}';",
                UTILS_FILENAME
            )))
            .join("\n")
    }
}
impl<P: Into<PathBuf>> From<P> for ImportPaths {
    fn from(source: P) -> Self {
        Self(vec![source.into()])
    }
}

mod resolver {
    // TODO: There might be an easier way of doing this...
    use super::*;
    #[derive(Trace)]
    pub struct Resolver {
        inner: jrsonnet_evaluator::FileImportResolver,
        utils: Vec<u8>,
    }
    impl Resolver {
        pub fn new(paths: ImportPaths) -> Self {
            Self {
                inner: jrsonnet_evaluator::FileImportResolver::new(paths.0),
                utils: include_bytes!("../utils.libsonnet").into(),
            }
        }
    }
    impl jrsonnet_evaluator::ImportResolver for Resolver {
        fn resolve_from(
            &self,
            from: &SourcePath,
            path: &str,
        ) -> jrsonnet_evaluator::Result<SourcePath> {
            if path == UTILS_FILENAME {
                return Ok(SourcePath::new(jrsonnet_parser::SourceFile::new(
                    UTILS_FILENAME.into(),
                )));
            }
            self.inner.resolve_from(from, path)
        }
        fn resolve_from_default(&self, path: &str) -> jrsonnet_evaluator::Result<SourcePath> {
            self.inner.resolve_from_default(path)
        }
        fn resolve(&self, path: &Path) -> jrsonnet_evaluator::Result<SourcePath> {
            self.inner.resolve(path)
        }
        fn load_file_contents(&self, resolved: &SourcePath) -> jrsonnet_evaluator::Result<Vec<u8>> {
            if resolved
                .path()
                .map_or(false, |p| p == Path::new(UTILS_FILENAME))
            {
                return Ok(self.utils.clone());
            }
            self.inner.load_file_contents(resolved)
        }
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
    }
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

#[derive(Default)]
struct JsonnetFormatter<'a> {
    inner: serde_json::ser::PrettyFormatter<'a>,
    in_key: bool,
}
impl<'a> serde_json::ser::Formatter for JsonnetFormatter<'a> {
    fn begin_array<W>(&mut self, writer: &mut W) -> std::io::Result<()>
    where
        W: ?Sized + std::io::Write,
    {
        self.inner.begin_array(writer)
    }

    fn end_array<W>(&mut self, writer: &mut W) -> std::io::Result<()>
    where
        W: ?Sized + std::io::Write,
    {
        self.inner.end_array(writer)
    }

    fn begin_array_value<W>(&mut self, writer: &mut W, first: bool) -> std::io::Result<()>
    where
        W: ?Sized + std::io::Write,
    {
        self.inner.begin_array_value(writer, first)
    }
    fn end_array_value<W>(&mut self, writer: &mut W) -> std::io::Result<()>
    where
        W: ?Sized + std::io::Write,
    {
        self.inner.end_array_value(writer)
    }
    fn begin_object<W>(&mut self, writer: &mut W) -> std::io::Result<()>
    where
        W: ?Sized + std::io::Write,
    {
        self.inner.begin_object(writer)
    }

    fn end_object<W>(&mut self, writer: &mut W) -> std::io::Result<()>
    where
        W: ?Sized + std::io::Write,
    {
        self.inner.end_object(writer)
    }

    fn begin_object_key<W>(&mut self, writer: &mut W, first: bool) -> std::io::Result<()>
    where
        W: ?Sized + std::io::Write,
    {
        self.in_key = true;
        self.inner.begin_object_key(writer, first)
    }

    fn begin_object_value<W>(&mut self, writer: &mut W) -> std::io::Result<()>
    where
        W: ?Sized + std::io::Write,
    {
        self.in_key = false;
        self.inner.begin_object_value(writer)
    }

    fn end_object_value<W>(&mut self, writer: &mut W) -> std::io::Result<()>
    where
        W: ?Sized + std::io::Write,
    {
        self.inner.end_array_value(writer)
    }

    fn begin_string<W>(&mut self, writer: &mut W) -> std::io::Result<()>
    where
        W: ?Sized + std::io::Write,
    {
        if !self.in_key {
            write!(writer, "\"")?;
        }
        Ok(())
    }
    fn end_string<W>(&mut self, writer: &mut W) -> std::io::Result<()>
    where
        W: ?Sized + std::io::Write,
    {
        if !self.in_key {
            write!(writer, "\"")?;
        }
        Ok(())
    }
}
