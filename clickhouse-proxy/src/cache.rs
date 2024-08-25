// TODO:
// - Set a timeout on queries, to avoid waiting forever on stuck queries.
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::http;
use axum::response::IntoResponse;
use bincode::Options;
use futures::stream::StreamExt;
use serde::{Deserialize, Serialize};
use tokio::io::AsyncWriteExt;
use tokio::sync::Mutex;
use tracing::*;

use super::error::Error;
use super::PreparedRequest;
use crate::ClickhouseQuery;

const LIMIT_BYTES: usize = 10_000_000;

#[derive(thiserror::Error, Debug)]
pub enum CacheError {
    #[error("I/O error: {0}")]
    IO(#[from] std::io::Error),
    #[error("Serialization error: {0}")]
    Serialization(#[from] bincode::Error),
    #[error("Failed to send buffer for serialization")]
    SendBuf,
    #[error("Failed to convert response: {0}")]
    ConvertResponse(http::Error),
}

#[derive(PartialEq, Default)]
enum EntryStatus {
    #[default]
    None,
    Processed,
    TooLarge,
}
pub struct Cache {
    path: PathBuf,
    entries: Mutex<HashMap<u64, Arc<Mutex<EntryStatus>>>>,
    expiry: Option<chrono::Duration>,
}
/// The data is serialized as
///   Response header (this struct)
///   Actual response
/// in bincode  
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Response {
    query: Option<ClickhouseQuery>,
    headers: HashMap<String, String>,
    status: u16,
    date: chrono::DateTime<chrono::Utc>,
}
impl Response {
    fn bincode_options() -> impl bincode::config::Options {
        bincode::DefaultOptions::new()
            // Limit at 100 MB
            .with_limit(100_000_000)
            .allow_trailing_bytes()
    }
    async fn write_adapt(
        mut request: PreparedRequest,
        filename: &Path,
        mut guard: tokio::sync::OwnedMutexGuard<EntryStatus>,
    ) -> Result<axum::response::Response, Error> {
        let query = request.take_query();
        let resp = request.send().await?;

        let filename = filename.to_owned();
        let filename_tmp = filename.with_extension("tmp");
        let mut writer = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&filename_tmp)
            .unwrap();
        // Serialize header
        let header = Self {
            status: resp.status().as_u16(),
            date: chrono::Utc::now(),
            query,
            headers: resp
                .headers()
                .into_iter()
                .filter(|(k, _)| !k.as_str().starts_with("date"))
                .filter_map(|(k, v)| Some((k.as_str().to_string(), v.to_str().ok()?.to_string())))
                .collect(),
        };
        Self::bincode_options()
            .serialize_into(&mut writer, &header)
            .map_err(CacheError::Serialization)?;

        let mut bw = tokio::io::BufWriter::new(tokio::fs::File::from(writer));
        // Adapted response
        let resp2 = header.builder().header("X-Cache", "MISS");
        let body = resp.bytes_stream();

        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<bytes::Bytes>();
        // Writing task
        tokio::task::spawn(async move {
            let mut size = 0;
            while let Some(rx) = rx.recv().await {
                size += rx.len();
                if size <= LIMIT_BYTES {
                    bw.write_all(&rx).await?;
                }
            }
            if size > LIMIT_BYTES {
                metrics::counter!("large-responses").increment(1);
                warn!(size, "Not caching a large response");
                drop(bw);
                *guard = EntryStatus::TooLarge;
                let _ = std::fs::remove_file(filename_tmp);
            } else {
                bw.flush().await?;
                std::fs::rename(filename_tmp, filename)?;
            }
            *guard = EntryStatus::Processed;
            // Remove processing mark
            drop(guard);
            metrics::counter!("response-size").increment(size as u64);
            Ok::<_, CacheError>(())
        });
        let body = axum::body::Body::from_stream(body.map(move |buf| {
            let buf = buf.map_err(clickhouse_client::Error::from)?;
            tx.send(buf.clone()).map_err(|_| CacheError::SendBuf)?;
            Ok::<_, Error>(buf)
        }));
        Ok(resp2.body(body).map_err(CacheError::ConvertResponse)?)
    }

    fn builder(&self) -> http::response::Builder {
        let mut resp2 = axum::http::Response::builder().status(self.status);
        for (k, v) in &self.headers {
            resp2 = resp2.header(k, v);
        }
        resp2
    }
    #[allow(dead_code)]
    fn read_header(filename: &Path) -> Result<Self, CacheError> {
        let mut reader = std::fs::File::open(filename)?;
        Ok(Self::bincode_options().deserialize_from(&mut reader)?)
    }
    fn age(&self) -> chrono::Duration {
        chrono::Utc::now() - self.date
    }
    async fn read(filename: &Path) -> Result<(Self, axum::response::Response), CacheError> {
        let mut reader = std::fs::File::open(filename)?;
        let header: Self = Self::bincode_options().deserialize_from(&mut reader)?;
        let builder = header
            .builder()
            .header("X-Cache", "HIT")
            .header("Age", header.age().num_seconds());
        let reader = tokio::io::BufReader::new(tokio::fs::File::from(reader));
        let reader = tokio_util::io::ReaderStream::new(reader);
        builder
            .body(axum::body::Body::from_stream(reader))
            .map_err(CacheError::ConvertResponse)
            .map(|r| (header, r))
    }
}

impl Cache {
    pub fn init(path: &Path, expiry: Option<chrono::Duration>) -> Result<Self, Error> {
        info!("Initializing cache");
        std::fs::create_dir_all(path).map_err(CacheError::from)?;
        let s = Self {
            path: path.into(),
            entries: Default::default(),
            expiry,
        };
        Ok(s)
    }
    #[allow(dead_code)]
    fn list(&self) -> impl Iterator<Item = (Result<Response, CacheError>, PathBuf)> {
        glob::glob(&self.path.join("*").to_string_lossy())
            .unwrap()
            .filter_map(|f| f.ok())
            .map(|f| (Response::read_header(&f), f))
    }
    #[tracing::instrument(name = "cache", skip(self))]
    pub async fn process(
        &self,
        request: PreparedRequest,
    ) -> Result<axum::response::Response, Error> {
        let id = request.id;
        let filename = self.path.join(id.to_string());
        let entry = self.entries.lock().await.entry(id).or_default().clone();
        // This will block in case this entry is already processing (reading or writing).
        let mut entry = entry.lock_owned().await;
        if *entry == EntryStatus::TooLarge {
            warn!("This query previously failed to cache from being too large. Returning directly");
            return Ok(http::response::Response::from(request.send().await?).into_response());
        } else if *entry == EntryStatus::Processed {
            match Response::read(&filename).await {
                Ok((header, resp)) => {
                    if self
                        .expiry
                        .as_ref()
                        // Only return if not expired
                        .map_or(true, |exp| header.age() <= *exp)
                    {
                        metrics::counter!("cache-hits").increment(1);
                        return Ok(resp);
                    }
                }
                Err(e) => {
                    warn!("Failed to reach from cache, processing again: {}", e);
                    *entry = EntryStatus::default();
                }
            }
        }
        metrics::counter!("cache-misses").increment(1);
        Response::write_adapt(request, &filename, entry).await
    }
}
