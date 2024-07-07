use miette::Diagnostic;
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
    location: Option<[usize; 2]>,
}
impl From<sqlsonnet::Error> for Error {
    fn from(source: sqlsonnet::Error) -> Self {
        Self {
            message: source.to_string(),
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
