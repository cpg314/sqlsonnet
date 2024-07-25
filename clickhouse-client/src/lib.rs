use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Clone)]
pub struct HttpClient {
    client: reqwest::Client,
    url: reqwest::Url,
}

#[derive(Clone, Debug, Serialize, Deserialize, Hash)]
pub struct ClickhouseQuery {
    pub query: String,
    pub params: BTreeMap<String, String>,
}

impl HttpClient {
    pub fn new(url: reqwest::Url) -> Self {
        Self {
            url,
            client: reqwest::Client::new(),
        }
    }
    pub fn prepare_request(&self, query: &ClickhouseQuery) -> reqwest::RequestBuilder {
        self.client
            .post(self.url.clone())
            .body(query.query.clone())
            .query(&query.params)
            .header(reqwest::header::TRANSFER_ENCODING, "chunked")
    }
    pub async fn send_query(
        &self,
        query: &ClickhouseQuery,
    ) -> Result<reqwest::Response, reqwest::Error> {
        let request = self.prepare_request(query);
        request.send().await?.error_for_status()
    }
}
