use wasm_bindgen::prelude::*;

use sqlsonnet::{jsonnet::FsResolver, Query};

#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

pub fn set_panic_hook() {
    console_error_panic_hook::set_once();
}

pub struct Error(sqlsonnet::Error);

impl From<sqlsonnet::Error> for Error {
    fn from(value: sqlsonnet::Error) -> Self {
        Self(value)
    }
}
impl From<Error> for JsValue {
    fn from(source: Error) -> Self {
        serde_wasm_bindgen::to_value(&source.0.formatted()).unwrap()
    }
}

#[wasm_bindgen]
pub fn to_sql(input: &str) -> Result<String, Error> {
    let query = Query::from_jsonnet(
        &format!("{}\n{}", sqlsonnet::jsonnet::import_utils(), input),
        sqlsonnet::jsonnet::Options::new(
            FsResolver::default(),
            concat!(env!("CARGO_CRATE_NAME"), " ", env!("CARGO_PKG_VERSION")),
        ),
    )?;
    Ok(query.to_sql(false))
}
