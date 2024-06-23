use pretty_assertions::assert_eq;
use sqlsonnet::Queries;

#[test]
fn sql_roundtrip() -> anyhow::Result<()> {
    // SQL to Query
    let input = include_str!("test.sql");
    let queries = Queries::from_sql(input)?;
    println!("{:?}", queries);

    // Queries to SQL
    let sql = queries.to_sql();
    println!("{}", sql);

    assert_eq!(sql.trim(), input.trim());

    // Queries to Jsonnet
    let jsonnet = queries.as_jsonnet().to_string();
    println!("{}", jsonnet);

    // Jsonnet to queries
    let queries2 = Queries::from_jsonnet(&jsonnet)?;
    assert_eq!(queries, queries2);

    Ok(())
}
