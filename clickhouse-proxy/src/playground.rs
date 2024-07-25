use std::collections::{BTreeMap, HashMap};
use std::hash::{DefaultHasher, Hash, Hasher};

use regex::Replacer;
use serde::{Deserialize, Serialize};
use tokio::io::AsyncWriteExt;
use tracing::*;

use super::{decode_query, ClickhouseQuery, Error, SharingError, State};

const ROWS_LIMIT: usize = 20;
const RESP_LIMIT: usize = 10_000_000;
const FORMAT: &str = "PrettyMonoBlock";

static PROJECT_DIR: include_dir::Dir<'_> = include_dir::include_dir!("playground/dist-proxy");

lazy_static::lazy_static! {
    pub static ref VARIABLE_PLACEHOLDER_RE: regex::Regex = {
        regex::Regex::new(r#"JSONNETFMT(\d+)"#).unwrap()
    };
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

struct IndexReplacer<F: Fn(usize) -> String> {
    i: usize,
    f: F,
}
impl<F: Fn(usize) -> String> Replacer for IndexReplacer<F> {
    fn replace_append(&mut self, _caps: &regex::Captures<'_>, dst: &mut String) {
        dst.push_str(&(self.f)(self.i));
        self.i += 1;
    }
}
async fn jsonnetfmt(data: &str) -> anyhow::Result<String> {
    // Replace Grafana variables by placeholders
    let variables: Vec<_> = VARIABLE_RE.captures_iter(data).collect();
    let data = VARIABLE_RE.replace_all(
        data,
        IndexReplacer {
            i: 0,
            f: |i| format!("JSONNETFMT{}", i),
        },
    );

    let mut process = tokio::process::Command::new("jsonnetfmt")
        .arg("-")
        .stdin(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()?;
    let mut stdin = process.stdin.take().unwrap();
    stdin.write_all(data.as_bytes()).await?;
    drop(stdin);
    let output = process.wait_with_output().await?;
    anyhow::ensure!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let out = String::from_utf8(output.stdout)?;

    // Put back the variables
    let out = VARIABLE_PLACEHOLDER_RE
        .replace_all(
            &out,
            IndexReplacer {
                i: 0,
                f: |i| variables[i].get(0).unwrap().as_str().to_string(),
            },
        )
        .to_string();

    Ok(out)
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
        #[serde(default)]
        format: bool,
    }
    impl Message {
        fn decode(message: Result<ws::Message, axum::Error>) -> Result<Self, WebsocketError> {
            Ok(serde_json::from_str(&message?.into_text()?)?)
        }
        // Replace ${var} by var.
        // In Grafana, u.string(${var:singlequote}) will be replaced by u.string('value'),
        // which is '"' + 'value' + '"'
        fn replace_variables(mut self) -> Self {
            self.jsonnet = VARIABLE_RE
                .replace_all(&self.jsonnet, |caps: &regex::Captures| {
                    caps.get(1).unwrap().as_str().to_string()
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
        let message = Message::decode(message)?;
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
        if message.format {
            return Ok(Response {
                initial: Some(
                    jsonnetfmt(&message.jsonnet)
                        .await
                        .map_err(|e| Error::JsonnetFmt(e.to_string()))?,
                ),
                ..Default::default()
            });
        };
        let message = message.replace_variables();
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
            assert_eq!(msg.jsonnet, "a b");
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
