use std::sync::Arc;

use axum::{extract::State, response::IntoResponse};
use futures::lock::Mutex;

async fn handler(State(last): State<LastQuery>, query: String) -> axum::response::Response {
    *last.lock().await = query;
    "2".into_response()
}
type LastQuery = Arc<Mutex<String>>;
#[tokio::test]
async fn main() -> anyhow::Result<()> {
    let last_query = LastQuery::default();
    // Spin up a fake Clickhouse server
    let fake_ch = axum::Router::new()
        .route("/", axum::routing::post(handler))
        .with_state(last_query.clone());
    let listener = tokio::net::TcpListener::bind("0.0.0.0:1235").await?;
    let fake_chaddr = listener.local_addr()?;
    tokio::spawn(async move {
        axum::serve(listener, fake_ch).await.unwrap();
    });

    // Start proxy
    let listener = tokio::net::TcpListener::bind("0.0.0.0:0").await?;
    let port = listener.local_addr()?.port();
    drop(listener);
    let cache = tempfile::tempdir()?;
    let _server = tokio::spawn(clickhouse_proxy::main_impl(clickhouse_proxy::Flags {
        url: reqwest::Url::parse(&format!("http://{}", fake_chaddr))?,
        username: "default".into(),
        password: None,
        cache: Some(cache.path().into()),
        library: None,
        prelude: None,
        shares: None,
        port,
    }));

    // Wait until the server is up
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    // Setup a client
    struct Client {
        url: reqwest::Url,
        client: reqwest::Client,
    }
    impl Client {
        async fn send(&self, query: &str) -> reqwest::Result<(String, reqwest::header::HeaderMap)> {
            let resp = self
                .client
                .post(self.url.clone())
                .body(query.to_owned())
                .send()
                .await?
                .error_for_status()?;
            let headers = resp.headers().clone();
            Ok((resp.text().await?, headers))
        }
    }
    let mut url = reqwest::Url::parse("http://localhost")?;
    url.set_port(Some(port)).unwrap();
    let client = Client {
        url,
        client: reqwest::Client::new(),
    };

    // Sending SQL
    assert_eq!(client.send("SELECT 1+1").await?.0, "2");

    let query = sqlsonnet::Query::from_sql("SELECT count(*) AS c FROM table")?;
    for i in 0..2 {
        // Sending Jsonnet
        let headers = client.send(&query.as_jsonnet().to_string()).await?.1;
        assert_eq!(
            headers.get("X-Cache").unwrap().to_str()?,
            if i == 0 { "MISS" } else { "HIT" }
        );
        assert_eq!(last_query.lock().await.as_str(), query.to_sql(true));
    }

    // Using the standard library
    client
        .send(r#"{ select: { from: "table", fields: [u.count()] } }"#)
        .await?;
    assert_eq!(last_query.lock().await.as_str(), query.to_sql(true));

    Ok(())
}
