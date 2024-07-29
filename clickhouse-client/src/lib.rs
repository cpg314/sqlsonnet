use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

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
    Gzip,
}
impl Compression {
    fn name(&self) -> &'static str {
        match self {
            Compression::None => "",
            Compression::Zstd => "zstd",
            Compression::Gzip => "gzip",
        }
    }
    pub fn from_headers(hm: &reqwest::header::HeaderMap) -> Self {
        hm.get(reqwest::header::ACCEPT_ENCODING)
            .and_then(|h| h.to_str().ok())
            .and_then(|h| {
                // TODO: Do this in a compliant manner
                if h.contains("zstd") {
                    Some(Self::Zstd)
                } else if h.contains("gzip") {
                    Some(Self::Gzip)
                } else {
                    None
                }
            })
            .unwrap_or(Self::None)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Hash)]
pub struct ClickhouseQuery {
    pub query: String,
    pub params: BTreeMap<String, String>,
    pub compression: Compression,
}

pub struct PreparedRequest(reqwest::RequestBuilder);
impl PreparedRequest {
    pub async fn send(self) -> Result<reqwest::Response, Error> {
        let resp = self.0.send().await?;
        if !resp.status().is_success() {
            return Err(Error::Clickhouse(resp.text().await.unwrap_or_default()));
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
        println!("{:?}", query.compression);
        builder = builder.header(reqwest::header::ACCEPT_ENCODING, query.compression.name());
        PreparedRequest(builder)
    }
    pub async fn send_query(&self, query: &ClickhouseQuery) -> Result<reqwest::Response, Error> {
        self.prepare_request(query).send().await
    }
}
