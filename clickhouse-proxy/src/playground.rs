use std::collections::{BTreeMap, HashMap};

use serde::Serialize;
use uuid::Uuid;

use super::{decode_query, ClickhouseQuery, Error, SharingError, State};

const ROWS_LIMIT: usize = 20;
const RESP_LIMIT: usize = 10_000_000;
const FORMAT: &str = "PrettyMonoBlock";

pub fn router() -> axum::Router<State> {
    axum::Router::new()
        .route("/", axum::routing::post(playground_post))
        .route("/", axum::routing::get(playground))
        .route("/share", axum::routing::post(share))
        .route(
            "/jsonnet.js",
            axum::routing::get(|| async { include_str!("jsonnet.js") }),
        )
}

async fn share(
    axum::extract::State(state): axum::extract::State<State>,
    data: String,
) -> Result<String, SharingError> {
    let id = uuid::Uuid::new_v4().to_string();
    let shares = state.args.shares.as_ref().ok_or(SharingError::NotEnabled)?;
    std::fs::create_dir_all(shares)?;
    std::fs::write(shares.join(&id), data.trim())?;
    Ok(id)
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
fn read_share(id: &str, state: State) -> Result<String, SharingError> {
    let shares = state.args.shares.as_ref().ok_or(SharingError::NotEnabled)?;
    Uuid::try_parse(id)?;
    Ok(std::fs::read_to_string(shares.join(id))?)
}
async fn playground(
    axum::extract::State(state): axum::extract::State<State>,
    axum::extract::Query(query): axum::extract::Query<HashMap<String, String>>,
) -> Result<axum::response::Html<String>, Error> {
    let initial = if let Some(share) = query.get("share") {
        &read_share(share, state.clone())?
    } else {
        include_str!("initial.jsonnet")
    };
    let prelude = state.prelude()?;
    let prelude = prelude.trim();
    let html = include_str!("playground.html")
        .replace("[PRELUDE]", prelude)
        .replace("[INITIAL]", initial.trim())
        .replace("[PRELUDE_ROWS]", &prelude.lines().count().to_string());
    Ok(axum::response::Html(html))
}
