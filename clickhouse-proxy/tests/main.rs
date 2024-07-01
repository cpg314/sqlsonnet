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

    // Start our proxy server
    let binary = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("sqlsonnet_clickhouse_proxy");
    let listener = tokio::net::TcpListener::bind("0.0.0.0:0").await?;
    let port = listener.local_addr()?.port();
    drop(listener);
    let _handle = tokio::process::Command::new(binary)
        .args([
            "--port",
            &port.to_string(),
            "--username",
            "default",
            "--url",
            &("http://".to_string() + &fake_chaddr.to_string()),
        ])
        .kill_on_drop(true)
        .spawn()?;
    // Wait until the server is up
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    // Setup a client
    struct Client {
        url: reqwest::Url,
        client: reqwest::Client,
    }
    impl Client {
        async fn send(&self, query: &str) -> reqwest::Result<String> {
            self.client
                .post(self.url.clone())
                .body(query.to_owned())
                .send()
                .await?
                .error_for_status()?
                .text()
                .await
        }
    }
    let mut url = reqwest::Url::parse("http://localhost")?;
    url.set_port(Some(port)).unwrap();
    let client = Client {
        url,
        client: reqwest::Client::new(),
    };

    // Sending SQL
    assert_eq!(client.send("SELECT 1+1").await?, "2");

    // Sending Jsonnet
    let query = sqlsonnet::Query::from_sql("SELECT count(*) AS c FROM table")?;
    client.send(&query.as_jsonnet().to_string()).await?;
    assert_eq!(last_query.lock().await.as_str(), query.to_sql(true));

    // Using the standard library
    client
        .send(r#"{ select: { from: "table", fields: [u.count()] } }"#)
        .await?;
    assert_eq!(last_query.lock().await.as_str(), query.to_sql(true));

    Ok(())
}
