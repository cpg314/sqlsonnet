/// Integration test which requires a Clickhouse server running (use `cargo make docker-compose`)
#[tokio::test]
async fn integration() -> anyhow::Result<()> {
    // Start proxy
    let listener = tokio::net::TcpListener::bind("0.0.0.0:0").await?;
    let port = listener.local_addr()?.port();
    drop(listener);
    let cache = tempfile::tempdir()?;
    let _server = tokio::spawn(clickhouse_proxy::main_impl(clickhouse_proxy::Flags {
        url: reqwest::Url::parse("http://default@localhost:8123")?,
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
        "2\n"
    );

    // Cache
    let query = sqlsonnet::Query::from_sql("SELECT count(*) AS c FROM system.one")?;
    for i in 0..2 {
        // Sending Jsonnet
        let resp = client
            .send_query(&query.as_jsonnet().to_string().as_str().into())
            .await?;
        assert_eq!(
            resp.headers().get("X-Cache").unwrap().to_str()?,
            if i == 0 { "MISS" } else { "HIT" }
        );
        assert_eq!(resp.text().await?, "1\n");
    }

    // Using the standard library
    let out = client
        .send_query(&r#"{ select: { from: "system.one", fields: [u.count()] } }"#.into())
        .await?
        .text()
        .await?;
    assert_eq!(out, "1\n");

    Ok(())
}
