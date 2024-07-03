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

fn run_query(q: &str) -> anyhow::Result<Query> {
    Ok(Query::from_jsonnet(
        &format!("local u = import 'sqlsonnet.libsonnet'; {}", q),
        FsResolver::default(),
    )?)
}

#[test]
fn function_call() -> anyhow::Result<()> {
    let query = run_query("{ select: { fields: [ u.fn('test', [1, 2]) ] } }")?;
    assert_eq!(query.to_sql(true), "SELECT test(1, 2)");

    Ok(())
}

#[test]
fn parenthesization() -> anyhow::Result<()> {
    let query = run_query("{ select: { fields: [ u.op('*', [3, u.op('+', [1, 2])]) ] } }")?;
    assert_eq!(query.to_sql(true), "SELECT 3 * (1 + 2)");

    Ok(())
}
