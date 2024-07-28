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
pub struct ClickhouseQuery {
    pub query: String,
    pub params: BTreeMap<String, String>,
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
    pub fn new(url: reqwest::Url) -> Self {
        Self {
            url,
            client: reqwest::Client::new(),
        }
    }
    pub fn prepare_request(&self, query: &ClickhouseQuery) -> PreparedRequest {
        PreparedRequest(
            self.client
                .post(self.url.clone())
                .body(query.query.clone())
                .query(&query.params)
                .header(reqwest::header::TRANSFER_ENCODING, "chunked"),
        )
    }
    pub async fn send_query(&self, query: &ClickhouseQuery) -> Result<reqwest::Response, Error> {
        self.prepare_request(query).send().await
    }
}
