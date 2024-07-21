use std::collections::{BTreeMap, HashMap};
use std::hash::{DefaultHasher, Hash, Hasher};

use serde::{Deserialize, Serialize};
use tracing::*;

use super::{decode_query, ClickhouseQuery, Error, SharingError, State};

const ROWS_LIMIT: usize = 20;
const RESP_LIMIT: usize = 10_000_000;
const FORMAT: &str = "PrettyMonoBlock";

static PROJECT_DIR: include_dir::Dir<'_> = include_dir::include_dir!("playground/dist-proxy");

lazy_static::lazy_static! {
    pub static ref VARIABLE_RE: regex::Regex = {
        regex::Regex::new(r#"\$\{([a-z_A-Z0-9]+)(:[a-z]+)?\}"#).unwrap()
    };
}

pub fn router() -> axum::Router<State> {
    axum::Router::new()
        .route(
            "/",
            axum::routing::get(
                || async move { serve(axum::extract::Path("index.html".into())).await },
            ),
        )
        .route("/*path", axum::routing::get(serve))
        .route("/ws", axum::routing::get(websocket::get))
}

async fn serve(
    axum::extract::Path(path): axum::extract::Path<String>,
) -> Result<impl axum::response::IntoResponse, axum::http::StatusCode> {
    let file = PROJECT_DIR
        .get_file(&path)
        .ok_or(axum::http::StatusCode::NOT_FOUND)?;
    let mime = mime_guess::from_path(path)
        .first_or_octet_stream()
        .to_string();
    Ok(([(axum::http::header::CONTENT_TYPE, mime)], file.contents()))
}

mod websocket {
    use super::*;
    use crate::error::WebsocketError;
    use axum::extract::ws;
    use axum::response::IntoResponse;

    #[derive(Default, Deserialize)]
    struct Message {
        jsonnet: String,
        // Send the query to Clickhouse
        #[serde(default)]
        clickhouse: bool,
        // Create a share
        #[serde(default)]
        share: bool,
    }
    impl Message {
        fn decode(message: Result<ws::Message, axum::Error>) -> Result<Self, WebsocketError> {
            Ok(serde_json::from_str(&message?.into_text()?)?)
        }
        fn replace_variables(mut self) -> Self {
            self.jsonnet = VARIABLE_RE
                .replace_all(&self.jsonnet, |caps: &regex::Captures| {
                    let ident = caps.get(1).unwrap().as_str();
                    match caps.get(2).map(|c| c.as_str()) {
                        Some(":singlequote") => format!(r#""'" + {} + "'""#, ident),
                        _ => ident.to_string(),
                    }
                })
                .to_string();
            self
        }
    }
    #[derive(Default, Serialize)]
    struct Response {
        // Generated SQL
        sql: Option<String>,
        // Clickhouse response
        data: Option<String>,
        // Initial Jsonnet (prelude + share)
        initial: Option<String>,
        // Share ID
        share: Option<String>,
        error: Option<sqlsonnet::FormattedError>,
    }
    impl Response {
        fn encode(res: Result<Response, Error>) -> String {
            serde_json::to_string(
                &res.map_err(sqlsonnet::FormattedError::from)
                    .unwrap_or_else(|error| Response {
                        error: Some(error),
                        ..Default::default()
                    }),
            )
            .unwrap()
        }
    }
    pub async fn get(
        ws: ws::WebSocketUpgrade,
        axum::extract::Query(query): axum::extract::Query<HashMap<String, String>>,
        axum::extract::State(state): axum::extract::State<State>,
    ) -> Result<axum::response::Response, Error> {
        let initial = if let Some(share) = query.get("share") {
            read_share(share, state.clone())?
        } else {
            format!("{}\n{}", state.prelude()?, include_str!("initial.jsonnet"))
        };
        info!("Creating a new websocket connection");
        Ok(ws
            .on_upgrade(|mut socket| async move {
                if let Err(e) = socket
                    .send(
                        Response::encode(Ok(Response {
                            initial: Some(initial),
                            ..Default::default()
                        }))
                        .into(),
                    )
                    .await
                {
                    error!("Sending prelude to socket failed: {}", e);
                    return;
                }

                while let Some(msg) = socket.recv().await {
                    let state = state.clone();
                    let res = Response::encode(handle_message(msg, state).await);

                    if let Err(e) = socket.send(res.into()).await {
                        error!("Sending to socket failed: {}", e);
                        break;
                    }
                }
            })
            .into_response())
    }
    async fn handle_message(
        message: Result<ws::Message, axum::Error>,
        state: State,
    ) -> Result<Response, Error> {
        info!("Handling websocket message");
        // TODO: Do the CPU-bound operations in a thread
        let message = Message::decode(message)?.replace_variables();
        let sql = decode_query(&message.jsonnet, state.clone(), false, Some(ROWS_LIMIT))?;
        let data = if message.clickhouse {
            let resp = state
                .send_query(ClickhouseQuery {
                    query: sql.clone(),
                    params: BTreeMap::from([("default_format".into(), FORMAT.into())]),
                })
                .await?;
            let resp = axum::body::to_bytes(resp.into_body(), RESP_LIMIT)
                .await
                .map_err(Error::ConvertBody)?;
            Some(String::from_utf8_lossy(&resp).to_string())
        } else {
            None
        };
        let share = if message.share {
            let data = message.jsonnet.trim();
            let mut hasher = DefaultHasher::default();
            data.hash(&mut hasher);
            let id = hasher.finish().to_string();
            let shares = state.args.shares.as_ref().ok_or(SharingError::NotEnabled)?;
            std::fs::create_dir_all(shares).map_err(crate::SharingError::from)?;
            let dest = shares.join(&id);
            if !dest.exists() {
                std::fs::write(dest, data).map_err(crate::SharingError::from)?;
            }
            Some(id)
        } else {
            None
        };
        Ok(Response {
            sql: Some(sql),
            data,
            share,
            ..Default::default()
        })
    }

    mod test {
        #[test]
        fn variables() {
            let msg = super::Message {
                jsonnet: "${a} ${b:singlequote}".into(),
                ..Default::default()
            }
            .replace_variables();
            assert_eq!(msg.jsonnet, r#"a "'" + b + "'""#);
        }
    }
}

fn read_share(id: &str, state: State) -> Result<String, SharingError> {
    let shares = state.args.shares.as_ref().ok_or(SharingError::NotEnabled)?;
    if !id.chars().all(|c| c.is_alphanumeric() || c == '-') {
        return Err(SharingError::InvalidId);
    }
    Ok(std::fs::read_to_string(shares.join(id))?)
}
