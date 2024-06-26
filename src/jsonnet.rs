use jrsonnet_stdlib::StateExt;
use serde::Serialize;

use crate::error::JsonnetError;

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
        .map_err(|e| JsonnetError::from(Some(filename), src, e))
}

/// Evaluate Jsonnet into JSON
pub fn evaluate(jsonnet: &str) -> Result<String, crate::error::JsonnetError> {
    let state = jrsonnet_evaluator::State::default();
    state.with_stdlib();
    state.set_import_resolver(jrsonnet_evaluator::FileImportResolver::default());

    evaluate_snippet(
        "utils.libsonnet",
        include_str!("../utils.libsonnet"),
        &state,
    )?;

    let val = evaluate_snippet("input.jsonnet", jsonnet, &state)?;
    let format = Box::new(jrsonnet_evaluator::manifest::JsonFormat::cli(3));
    val.manifest(format)
        .map_err(|e| JsonnetError::from(None, jsonnet, e))
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
