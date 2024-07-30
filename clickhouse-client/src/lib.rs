use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use tracing::*;

#[derive(Clone)]
pub struct HttpClient {
    client: reqwest::Client,
    url: reqwest::Url,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Request error: {0}")]
    Request(#[from] reqwest::Error),
    #[error("Clickhouse error: {0}")]
    Clickhouse(String),
}

#[derive(Clone, Debug, Serialize, Deserialize, Hash)]
pub enum Compression {
    None,
    Zstd,
}
impl Compression {
    fn name(&self) -> &'static str {
        match self {
            Compression::None => "",
            Compression::Zstd => "zstd",
        }
    }
    pub fn from_headers(
        hm: &reqwest::header::HeaderMap,
        field: reqwest::header::HeaderName,
    ) -> Self {
        hm.get(field)
            .and_then(|h| h.to_str().ok())
            .and_then(|h| {
                // TODO: Do this in a compliant manner
                if h.contains("zstd") {
                    Some(Self::Zstd)
                } else {
                    None
                }
            })
            .unwrap_or(Self::None)
    }
    pub fn decode(&self, data: bytes::Bytes) -> Result<bytes::Bytes, std::io::Error> {
        match self {
            Compression::None => Ok(data),
            Compression::Zstd => {
                let cur = std::io::Cursor::new(data);
                Ok(zstd::decode_all(cur)?.into())
            }
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Hash)]
pub struct ClickhouseQuery {
    pub query: String,
    pub params: BTreeMap<String, String>,
    pub compression: Compression,
}
impl From<&str> for ClickhouseQuery {
    fn from(source: &str) -> Self {
        Self {
            query: source.into(),
            params: Default::default(),
            compression: Compression::None,
        }
    }
}

pub struct PreparedRequest(reqwest::RequestBuilder);
impl PreparedRequest {
    pub async fn send(self) -> Result<reqwest::Response, Error> {
        let resp = self.0.send().await?;
        if !resp.status().is_success() {
            // We decode the response manually if needed, because `decompression` was not
            // necessarily set (e.g. if we're proxying the encoded data)
            let encoding =
                Compression::from_headers(resp.headers(), reqwest::header::CONTENT_ENCODING);
            let data = encoding
                .decode(resp.bytes().await?)
                .inspect_err(|e| warn!("Failed to decode error: {}", e))
                .unwrap_or_default();
            return Err(Error::Clickhouse(
                String::from_utf8_lossy(&data).to_string(),
            ));
        }
        Ok(resp)
    }
}

impl HttpClient {
    pub fn new(url: reqwest::Url, decompression: bool) -> Self {
        Self {
            url,
            client: reqwest::ClientBuilder::new()
                .zstd(decompression)
                .gzip(decompression)
                .build()
                .unwrap(),
        }
    }
    pub fn prepare_request(&self, query: &ClickhouseQuery) -> PreparedRequest {
        let mut builder = self
            .client
            .post(self.url.clone())
            .body(query.query.clone())
            .query(&query.params)
            .header(reqwest::header::TRANSFER_ENCODING, "chunked");
        builder = builder.header(reqwest::header::ACCEPT_ENCODING, query.compression.name());
        PreparedRequest(builder)
    }
    pub async fn send_query(&self, query: &ClickhouseQuery) -> Result<reqwest::Response, Error> {
        self.prepare_request(query).send().await
    }
}
