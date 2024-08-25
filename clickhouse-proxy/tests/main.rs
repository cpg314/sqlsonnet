use std::collections::BTreeMap;

use tracing::*;

/// Integration test which requires a Clickhouse server running (use `cargo make docker-compose`)
#[tokio::test]
async fn integration() -> anyhow::Result<()> {
    let cache = tempfile::tempdir()?;
    let prelude = tempfile::NamedTempFile::new()?;
    std::fs::write(
        prelude.path(),
        sqlsonnet_macros::sqlsonnet_lit!(local u2 = import "sqlsonnet.libsonnet"; {}),
    )?;
    let library = tempfile::tempdir()?;
    std::fs::write(library.path().join("test.libsonnet"), "{ answer: 42 }")?;
    let other = library.path().join("other");
    std::fs::create_dir_all(&other)?;
    std::fs::write(other.join("test.libsonnet"), "{ answer: 50 }")?;

    // Start proxy
    let listener = tokio::net::TcpListener::bind("0.0.0.0:0").await?;
    let port = listener.local_addr()?.port();
    drop(listener);
    let _server = tokio::spawn(async move {
        if let Err(e) = clickhouse_proxy::main_impl(clickhouse_proxy::Flags {
            url: reqwest::Url::parse("http://default@localhost:8123").unwrap(),
            cache: Some(cache.path().into()),
            library: Some(vec![library.path().into()]),
            prelude: Some(prelude.path().into()),
            shares: None,
            port,
        })
        .await
        {
            error!("{}", e);
        }
    });

    // Wait until the server is up
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    // Setup client
    let mut url = reqwest::Url::parse("http://localhost")?;
    url.set_port(Some(port)).unwrap();
    let client = clickhouse_client::HttpClient::new(url.clone(), true);

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

    let query_lit = sqlsonnet_macros::sqlsonnet_lit!(
     local l = import "test.libsonnet";
     { select: { from: "system.one", fields: [l.answer, u2.count()] } }
    );
    let query: clickhouse_client::ClickhouseQuery = query_lit.into();
    // Using the embedded and custom libraries
    let out = client.send_query(&query).await?.text().await?;
    assert_eq!(out, "42\t1\n");
    // Changing the library with the jpath header
    let query = clickhouse_client::ClickhouseQuery {
        headers: BTreeMap::from([("jpath".to_string(), "other".to_string())]),
        ..query
    };
    let out = client.prepare_request(&query)?.send().await?.text().await?;
    assert_eq!(out, "50\t1\n");
    // Changing the library with the jpath comment
    // NOTE: We use a string instead of the sqlsonnet_lit macro so that the comment does not get
    //       eaten by rustc.
    let mut query: clickhouse_client::ClickhouseQuery =
        format!("//sqlsonnet-jpath: other \n {}", query_lit)
            .as_str()
            .into();
    query.headers = BTreeMap::from([("jpath".to_string(), "other".to_string())]);
    let out = client.prepare_request(&query)?.send().await?.text().await?;
    assert_eq!(out, "50\t1\n");

    // Check Prometheus metrics
    let metrics = reqwest::get(url.join("metrics")?)
        .await?
        .error_for_status()?
        .text()
        .await?;
    println!("{}", metrics);
    assert!(metrics.contains("cache_hits 2"));
    assert!(metrics.contains("cache_misses 4"));

    Ok(())
}
