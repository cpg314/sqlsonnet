use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::body::Body;
use axum::http;
use bincode::Options;
use bytes::Bytes;
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use tracing::*;

use super::error::{ClickhouseError, Error};
use super::PreparedRequest;
use crate::ClickhouseQuery;

const LIMIT_BYTES: u64 = 10_000_000;

#[derive(thiserror::Error, Debug)]
pub enum CacheError {
    #[error("I/O error: {0}")]
    IO(#[from] std::io::Error),
    #[error("Serialization error: {0}")]
    Serialization(#[from] bincode::Error),
    #[error("Failed to convert response: {0}")]
    ConvertResponse(http::Error),
    #[error("Response too large ({0} bytes)")]
    TooLarge(usize),
}

pub struct Cache {
    path: PathBuf,
    entries: Mutex<HashMap<u64, Arc<Mutex<()>>>>,
}
#[derive(Clone, Serialize, Deserialize)]
struct Response {
    query: Option<ClickhouseQuery>,
    body: Bytes,
    headers: HashMap<String, String>,
    status: u16,
    cached: bool,
    date: chrono::DateTime<chrono::Utc>,
}
impl Response {
    fn write(self, filename: &Path) -> Result<(), CacheError> {
        if self.body.len() as u64 > LIMIT_BYTES {
            return Err(CacheError::TooLarge(self.body.len()));
        }
        let writer = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(filename)?;
        let writer = std::io::BufWriter::new(writer);
        let options = bincode::DefaultOptions::new().with_limit(LIMIT_BYTES);
        if let Err(e) = options.serialize_into(writer, &self) {
            let _ = std::fs::remove_file(filename);
            return Err(e.into());
        }
        Ok(())
    }
    fn read(filename: &Path) -> Result<Self, CacheError> {
        let options = bincode::DefaultOptions::new().with_limit(LIMIT_BYTES);
        let reader = std::fs::File::open(filename)?;
        let reader = std::io::BufReader::new(reader);
        let mut r: Self = options.deserialize_from(reader)?;
        r.cached = true;
        Ok(r)
    }
    async fn from_response(
        query: Option<ClickhouseQuery>,
        resp: reqwest::Response,
    ) -> Result<Self, ClickhouseError> {
        Ok(Self {
            status: resp.status().as_u16(),
            date: chrono::Utc::now(),
            query,
            headers: resp
                .headers()
                .into_iter()
                .filter_map(|(k, v)| Some((k.as_str().to_string(), v.to_str().ok()?.to_string())))
                .collect(),
            cached: false,
            body: resp.bytes().await?,
        })
    }
}
impl TryFrom<Response> for http::Response<Body> {
    type Error = CacheError;
    fn try_from(value: Response) -> Result<Self, Self::Error> {
        let mut resp = axum::http::Response::builder().status(value.status);
        for (k, v) in value.headers {
            if k.starts_with("x-clickhouse") {
                resp = resp.header(k, v);
            }
        }
        resp = resp.header("X-Cache", if value.cached { "HIT" } else { "MISS" });
        resp = resp.header("Age", (chrono::Utc::now() - value.date).num_seconds());
        resp.body(Body::from(value.body))
            .map_err(CacheError::ConvertResponse)
    }
}
impl Cache {
    pub fn init(path: &Path) -> Result<Self, Error> {
        info!("Initializing cache");
        std::fs::create_dir_all(path).map_err(CacheError::from)?;
        Ok(Self {
            path: path.into(),
            entries: Default::default(),
        })
    }
    #[tracing::instrument(name = "cache", skip(self))]
    pub async fn process(
        &self,
        mut request: PreparedRequest,
    ) -> Result<axum::response::Response, Error> {
        let id = request.id;
        let entry = self.entries.lock().await.get(&id).cloned();
        if let Some(entry) = entry {
            info!("Already processing, waiting");
            let _ = entry.lock().await;
        }
        // Not already processing
        let filename = self.path.join(id.to_string());
        if filename.exists() {
            // Read from cache
            info!("Reading response from cache");
            match Response::read(&filename).and_then(|r| r.try_into()) {
                Ok(r) => {
                    return Ok(r);
                }
                Err(e) => {
                    warn!("Failed reading response from cache: {:?}", e);
                }
            }
        }
        // Process
        // Mark as processing
        let mutex = Arc::<Mutex<()>>::default();
        let guard = mutex.lock().await;
        self.entries.lock().await.insert(id, mutex.clone());
        let query = request.take_query();
        let resp = request.send().await?;
        // Write to cache
        let resp = match Response::from_response(query, resp).await {
            Ok(r) => r,
            Err(e) => {
                self.entries.lock().await.remove(&id);
                return Err(e.into());
            }
        };
        if resp.status == StatusCode::OK.as_u16() {
            let resp = resp.clone();
            match tokio::task::spawn_blocking(move || resp.write(&filename)).await {
                Ok(Ok(())) => {}
                Ok(Err(e)) => {
                    warn!("Writing to cache failed: {}", e);
                }
                Err(e) => {
                    warn!("Writing to cache panicked: {}", e);
                }
            }
        }
        // Remove processing mark
        self.entries.lock().await.remove(&id);
        drop(guard);
        // Return response
        Ok(resp.try_into()?)
    }
}
