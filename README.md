# sqlsonnet

> [!WARNING]  
> Work in progress.

Express SQL queries with a simple [Jsonnet](https://jsonnet.org/) representation, not dissimilar to the [MongoDB Query Language](https://www.mongodb.com/docs/manual/reference/), which can be easily templated using the [Jsonnet configuration language](https://jsonnet.org/learning/tutorial.html).

```jsonnet
select: {
  fields: ['bwv', u.count()],
  from: 'cantatas',
  groupBy: ['year'],
} + { limit: 10 }
```

```sql
SELECT bwv, count(*) AS c
FROM cantatas
GROUP BY year
LIMIT 10;
```

## Tools

### `sqlsonnet`

The `sqlsonnet` command line interface _converts Jsonnet statements to SQL_, and to a lesser extent from SQL.

```
Usage: sqlsonnet [OPTIONS] <COMMAND>

Commands:
  from-sql  Convert SQL to Jsonnet
  to-sql    Convert Jsonnet to SQL
  help      Print this message or the help of the given subcommand(s)

Options:
      --theme <THEME>  Color theme for syntax highlighting [env: SQLSONNET_THEME=Nord] [possible values: 1337, Coldark-Cold, Coldark-Dark, DarkNeon, Dracula, GitHub, "Monokai Extended", "Monokai Extended Bright", "Monokai Extended Light", "Monokai Extended Origin", Nord, OneHalfDark, OneHalfLight, "Solarized (dark)", "Solarized (light)", "Sublime Snazzy", TwoDark, "Visual Studio Dark+", ansi, base16, base16-256, gruvbox-dark, gruvbox-light, zenburn]
  -c, --compact        Compact SQL representation
  -h, --help           Print help
  -V, --version        Print version
```

Errors in the Jsonnet or in the JSON will be nicely reported thanks to [miette](https://docs.rs/miette/latest/miette/index.html).

#### Jsonnet to SQL

```text
Usage: sqlsonnet to-sql [OPTIONS] <INPUT>

Arguments:
  <INPUT>  Input file (path or - for stdin)

Options:
      --display-format <DISPLAY_FORMAT>
          Display the converted SQL, the intermediary Json, or the original Jsonnet [default: sql] [possible values: sql, jsonnet, json]
```

The [embedded utility functions](sqlsonnet/utils.libsonnet) are automatically imported as

```jsonnet
local u = import "sqlsonnet.libsonnet";
```

_Example_

```
$ sqlsonnet from-sql test.sql
$ cat test.sql | sqlsonnet from-sql -
$ # Piping into clickhouse client
$ sqlsonnet from-sql test.sql | clickhouse client -f PrettyMonoBlock --multiquery --host ... --user ...
```

#### SQL to Jsonnet

This mode is useful to discover the sqlsonnet syntax from SQL queries.

The parser is far from perfect. Expressions are parsed as long as subqueries are encountered; then they are simply represented as strings. The results do not use the [embedded utility functions](sqlsonnet/utils.libsonnet), which can significantly simplify expressions.

```
Usage: sqlsonnet from-sql [OPTIONS] <INPUT>

Arguments:
  <INPUT>  Input file (path or - for stdin)

Options:
      --display-format <DISPLAY_FORMAT>
          Display the converted Jsonnet output and/or the SQL roundtrip [default: jsonnet] [possible values: sql, jsonnet, json]
      --diff
          Convert back to SQL and print the differences with the original, if any
```

_Example_

```console
$ sqlsonnet to-sql test.jsonnet
$ cat test.jsonnet | sqlsonnet to-sql -
```

### `sqlsonnet_clickhouse_proxy`

```text
Reverse proxies a Clickhouse HTTP server, transforming Jsonnet or JSON queries into SQL

Usage: sqlsonnet_clickhouse_proxy

Options:
  -h, --help     Print help
  -V, --version  Print version
```
