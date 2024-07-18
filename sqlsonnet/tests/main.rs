use pretty_assertions::assert_eq;

use sqlsonnet::{FsResolver, Queries, Query};

#[test]
fn sql_roundtrip() -> anyhow::Result<()> {
    // SQL to Query
    let input = include_str!("data/test.sql");
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

// TODO: This would be simpler with a trait on Query/Queries.
macro_rules! run_impl {
    ($i: ident, $t:ty) => {
        fn $i(q: &str) -> anyhow::Result<$t> {
            Ok(<$t>::from_jsonnet(
                &format!("{} {}", sqlsonnet::import_utils(), q),
                FsResolver::default(),
            )?)
        }
    };
}
run_impl!(run_query, Query);
run_impl!(run_queries, Queries);

mod jsonnet_to_sql {
    use super::*;
    macro_rules! jsonnet_to_sql {
        ($filename: ident) => {
            #[test]
            fn $filename() -> anyhow::Result<()> {
                let data = include_str!(
                    concat!("data/", stringify!($filename), ".jsonnet")
                );
                let expected = data.lines().next().unwrap().trim_start_matches("//").trim();
                let query = run_query(&data)?;
                pretty_assertions::assert_eq!(query.to_sql(true), expected);

                Ok(())
            }
        };
        ($($filename:ident),+) => {
            $( jsonnet_to_sql!($filename); )+
        };
    }
    jsonnet_to_sql!(function_call, parenthesization, as_override);
}

#[test]
fn examples() -> anyhow::Result<()> {
    run_queries(include_str!("data/example.jsonnet"))?;
    Ok(())
}

// Deserialize Queries from a Query.
#[test]
fn queries_single() -> anyhow::Result<()> {
    let s = "{ select: { fields: [1] } }";
    let queries = run_queries(s)?;
    let query = run_query(s)?;
    assert_eq!(queries, vec![query].into());
    Ok(())
}
