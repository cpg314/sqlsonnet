use wasm_bindgen::prelude::*;

use sqlsonnet::{FsResolver, Query};

#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

pub fn set_panic_hook() {
    console_error_panic_hook::set_once();
}

#[wasm_bindgen]
pub fn to_sql(input: &str) -> Result<String, String> {
    let query = Query::from_jsonnet(
        &format!("{}{}", sqlsonnet::import_utils(), input),
        FsResolver::default(),
    )
    .map_err(|e| e.to_string())?;
    Ok(query.to_sql(false))
}
