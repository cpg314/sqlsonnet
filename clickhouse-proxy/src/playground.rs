use std::collections::BTreeMap;

use serde::Serialize;

use super::{decode_query, ClickhouseQuery, Error, State};

const ROWS_LIMIT: usize = 20;
const RESP_LIMIT: usize = 10_000_000;
const FORMAT: &str = "PrettyMonoBlock";

pub fn router() -> axum::Router<State> {
    axum::Router::new()
        .route("/", axum::routing::post(playground_post))
        .route("/", axum::routing::get(playground))
        .route(
            "/jsonnet.js",
            axum::routing::get(|| async { include_str!("jsonnet.js") }),
        )
}

#[derive(Serialize)]
struct PlaygroundResponse {
    sql: String,
    data: String,
}
async fn playground_post(
    axum::extract::State(state): axum::extract::State<State>,
    request: String,
) -> Result<axum::Json<PlaygroundResponse>, Error> {
    let sql = {
        let state = state.clone();
        tokio::task::spawn_blocking(move || decode_query(request, state, false, Some(ROWS_LIMIT)))
            .await??
    };
    let resp = state
        .send_query(ClickhouseQuery {
            query: sql.clone(),
            params: BTreeMap::from([("default_format".into(), FORMAT.into())]),
        })
        .await?;
    let resp = axum::body::to_bytes(resp.into_body(), RESP_LIMIT)
        .await
        .map_err(Error::ConvertBody)?;
    let data = String::from_utf8_lossy(&resp);
    Ok(axum::Json(PlaygroundResponse {
        sql,
        data: data.into(),
    }))
}
async fn playground(
    axum::extract::State(state): axum::extract::State<State>,
) -> Result<axum::response::Html<String>, Error> {
    let prelude = state.prelude()?;
    let html = include_str!("playground.html")
        .replace("[PRELUDE]", &prelude)
        .replace("[PRELUDE_ROWS]", &prelude.lines().count().to_string());
    Ok(axum::response::Html(html))
}
