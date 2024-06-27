# sqlsonnet

> [!WARNING]  
> Work in progress.

Express SQL queries with a simple [Jsonnet](https://jsonnet.org/) representation[^1], which can be easily templated using the [Jsonnet configuration language](https://jsonnet.org/learning/tutorial.html).

[^1]: not dissimilar to the [MongoDB Query Language](https://www.mongodb.com/docs/manual/reference/).

```jsonnet
select: {
  fields: ['bwv', count()],
  from: 'cantatas',
  limit: 10,
  groupBy: ['year'],
}
```

```sql
SELECT bwv, count(*) AS c
FROM cantatas
GROUP BY year
LIMIT 10;
```

## Tools

### `sqlsonnet`

The main goal of this tool is to convert Jsonnet statements to and from SQL.

#### Jsonnet to SQL

```text
Usage: sqlsonnet to-sql [OPTIONS] <INPUT>

Arguments:
  <INPUT>  Input file (path or - for stdin)

Options:
      --display-format <DISPLAY_FORMAT>
          Display the converted SQL, the intermediary Json, or the original Jsonnet [default: sql] [possible values: sql, jsonnet, json]
  -h, --help
          Print help

```

_Example_

```
$ sqlsonnet from-sql test.sql
$ cat test.sql | sqlsonnet from-sql -
$ # Piping into clickhouse client
$ sqlsonnet from-sql test.sql | clickhouse client -f PrettyMonoBlock --multiquery --host ... --user ...
```

#### SQL to Jsonnet

This mode is useful to discover the sqlsonnet syntax from SQL queries. The parser is far from perfect. Expressions are parsed as long as subqueries are encountered; then they are simply represented as strings.

```
Usage: sqlsonnet from-sql [OPTIONS] <INPUT>

Arguments:
  <INPUT>  Input file (path or - for stdin)

Options:
      --display-format <DISPLAY_FORMAT>
          Display the converted Jsonnet output and/or the SQL roundtrip [default: jsonnet] [possible values: sql, jsonnet, json]
      --diff
          Convert back to SQL and print the differences with the original, if any
  -h, --help
          Print help
```

_Example_

```console
$ sqlsonnet to-sql test.jsonnet
$ cat test.jsonnet | sqlsonnet to-sql -
```

Errors in the Jsonnet or in the JSON will be nicely reported thanks to [miette](https://docs.rs/miette/latest/miette/index.html).

### `sqlsonnet_clickhouse_proxy`

```text
Reverse proxies a Clickhouse HTTP server, transforming Jsonnet or JSON queries into SQL

Usage: sqlsonnet_clickhouse_proxy

Options:
  -h, --help     Print help
  -V, --version  Print version
```
