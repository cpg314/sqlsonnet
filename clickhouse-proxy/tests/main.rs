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
        url: reqwest::Url::parse(&format!("http://default@{}", fake_chaddr))?,
        cache: Some(cache.path().into()),
        library: None,
        prelude: None,
        shares: None,
        port,
    }));

    // Wait until the server is up
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    // Setup client
    let mut url = reqwest::Url::parse("http://localhost")?;
    url.set_port(Some(port)).unwrap();
    let client = clickhouse_client::HttpClient::new(url, true);

    // Sending SQL
    assert_eq!(
        client
            .send_query(&"SELECT 1+1".into())
            .await?
            .text()
            .await?,
        "2"
    );

    // Cache
    let query = sqlsonnet::Query::from_sql("SELECT count(*) AS c FROM table")?;
    for i in 0..2 {
        // Sending Jsonnet
        let resp = client
            .send_query(&query.as_jsonnet().to_string().as_str().into())
            .await?;
        assert_eq!(
            resp.headers().get("X-Cache").unwrap().to_str()?,
            if i == 0 { "MISS" } else { "HIT" }
        );
        assert_eq!(last_query.lock().await.as_str(), query.to_sql(true));
    }

    // Using the standard library
    client
        .send_query(&r#"{ select: { from: "table", fields: [u.count()] } }"#.into())
        .await?;
    assert_eq!(last_query.lock().await.as_str(), query.to_sql(true));

    Ok(())
}
