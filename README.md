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

```text
Usage: sqlsonnet [OPTIONS] <COMMAND>

Commands:
  from-sql  Convert SQL to Jsonnet
  to-sql    Convert Jsonnet to SQL
  help      Print this message or the help of the given subcommand(s)

Options:
      --theme <THEME>  Color theme for syntax highlighting [env: SQLSONNET_THEME=] [possible
                       values: 1337, Coldark-Cold, Coldark-Dark, DarkNeon, Dracula, GitHub,
                       "Monokai Extended", "Monokai Extended Bright", "Monokai Extended Light",
                       "Monokai Extended Origin", Nord, OneHalfDark, OneHalfLight, "Solarized
                       (dark)", "Solarized (light)", "Sublime Snazzy", TwoDark, "Visual Studio
                       Dark+", ansi, base16, base16-256, gruvbox-dark, gruvbox-light, zenburn]
  -h, --help           Print help
  -V, --version        Print version
```

```console
$ sqlsonnet from-sql test.sql
$ cat test.sql | sqlsonnet from-sql -
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
