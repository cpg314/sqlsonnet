use pretty_assertions::assert_eq;

use sqlsonnet::{FsResolver, Queries, Query};

#[test]
fn sql_roundtrip() -> anyhow::Result<()> {
    // SQL to Query
    let input = include_str!("test.sql");
    let queries = Queries::from_sql(input)?;
    println!("{:?}", queries);

    // Queries to SQL
    let sql = queries.to_sql(false);
    println!("{}", sql);

    assert_eq!(sql.trim(), input.trim());

    // Queries to Jsonnet
    let jsonnet = queries.as_jsonnet().to_string();
    println!("{}", jsonnet);

    // Jsonnet to queries
    let queries2 = Queries::from_jsonnet(&jsonnet, FsResolver::default())?;
    assert_eq!(queries, queries2);

    Ok(())
}

#[test]
fn function_call() -> anyhow::Result<()> {
    let query = Query::from_jsonnet(
        "{ select: { fields: [{ fn: 'test', params: [1, 2] }], from: 'a' } }",
        FsResolver::default(),
    )?;
    assert_eq!(query.to_sql(true), "SELECT test(1, 2) FROM a ");

    Ok(())
}
