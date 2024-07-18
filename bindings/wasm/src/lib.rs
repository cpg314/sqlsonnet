use itertools::Itertools;
use miette::{Diagnostic, SourceCode};
use wasm_bindgen::prelude::*;

use sqlsonnet::{FsResolver, Query};

#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

pub fn set_panic_hook() {
    console_error_panic_hook::set_once();
}

#[derive(serde::Serialize)]
pub struct Error {
    message: String,
    code: Option<String>,
    location: Option<[usize; 2]>,
}
impl From<sqlsonnet::Error> for Error {
    fn from(source: sqlsonnet::Error) -> Self {
        match &source {
            sqlsonnet::Error::Jsonnet(_) => {
                Self {
                    message: source.to_string(),
                    code: None,
                    location: if let (Some(source_code), Some(labels)) =
                        (source.source_code(), source.labels())
                    {
                        labels
                            .filter_map(|l| source_code.read_span(l.inner(), 0, 0).ok())
                            // Subtract 1 for the initial line
                            .map(|sc| [sc.line() - 1, sc.column()])
                            .next()
                    } else {
                        None
                    },
                }
            }
            sqlsonnet::Error::Json(json_source) => Self {
                message: source.to_string(),
                code: json_source
                    .src
                    .read_span(&miette::SourceSpan::new(json_source.span, 1), 2, 2)
                    .ok()
                    .and_then(|contents| String::from_utf8(contents.data().into()).ok())
                    .map(|code| {
                        let indent = code
                            .lines()
                            .filter(|l| !l.is_empty())
                            .map(|l| l.chars().take_while(|c| c.is_whitespace()).count())
                            .min()
                            .unwrap_or_default();
                        let indent: String = " ".repeat(indent);
                        code.lines()
                            .map(|l| l.strip_prefix(&indent).unwrap_or(l))
                            .join("\n")
                    }),
                location: None,
            },

            sqlsonnet::Error::SqlParse(_) => unreachable!(),
        }
    }
}

impl From<Error> for JsValue {
    fn from(source: Error) -> Self {
        serde_wasm_bindgen::to_value(&source).unwrap()
    }
}

#[wasm_bindgen]
pub fn to_sql(input: &str) -> Result<String, Error> {
    let query = Query::from_jsonnet(
        &format!("{}\n{}", sqlsonnet::import_utils(), input),
        FsResolver::default(),
    )?;
    Ok(query.to_sql(false))
}
