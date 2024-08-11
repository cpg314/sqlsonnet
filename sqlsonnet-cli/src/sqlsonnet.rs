use std::collections::BTreeMap;
use std::io::IsTerminal;
use std::path::PathBuf;

use clap::Parser;
use tracing::*;

use sqlsonnet::Queries;

lazy_static::lazy_static! {
    static ref THEMES: Vec<String> =
        bat::assets::HighlightingAssets::from_binary().themes().map(String::from).collect();
}

#[derive(Debug, miette::Diagnostic, thiserror::Error)]
enum Error {
    #[error("Failed to read input")]
    Input(#[from] clap_stdin::StdinError),
    #[error(transparent)]
    #[diagnostic(transparent)]
    Inner(#[from] sqlsonnet::Error),
    #[error("Failed to highlight SQL")]
    Bat(#[from] bat::error::Error),
    #[error(transparent)]
    Miette(#[from] miette::InstallError),
    #[error("Failed to execute query")]
    Clickhouse(#[from] clickhouse_client::Error),
    #[error(transparent)]
    Watch(#[from] notify_debouncer_mini::notify::Error),
}

#[derive(Parser)]
#[clap(version)]
struct Flags {
    /// Color theme for syntax highlighting
    #[clap(long, env = "SQLSONNET_THEME",
           value_parser=clap::builder::PossibleValuesParser::new(THEMES.iter().map(|s| s.as_str())))]
    theme: Option<String>,
    /// Compact SQL representation
    #[clap(long, short)]
    compact: bool,
    /// Input file (path or - for stdin).
    input: clap_stdin::FileOrStdin,
    /// Convert an SQL file into Jsonnet.
    #[clap(long, short)]
    from_sql: bool,
    /// With --from-sql: Convert back to SQL and print the differences with the original, if any.
    #[clap(long, requires = "from_sql")]
    diff: bool,
    #[clap(long, value_delimiter = ',')]
    display_format: Option<Vec<Language>>,
    /// Clickhouse HTTP URL, to execute queries
    #[clap(long, env = "SQLSONNET_CLICKHOUSE")]
    clickhouse_url: Option<reqwest::Url>,
    /// Send query to Clickhouse proxy (--proxy-url) for execution
    #[clap(long, short, conflicts_with = "from_sql", requires = "clickhouse_url")]
    execute: bool,
    /// Output format for execution
    #[clap(long, default_value = "PrettyMonoBlock")]
    execute_format: String,
    /// Watch for file changes
    #[clap(long, short)]
    watch: bool,
    /// Library path
    #[clap(long, short = 'J', env = "JSONNET_PATH", value_delimiter = ':')]
    jpath: Option<Vec<PathBuf>>,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, clap::ValueEnum)]
enum Language {
    Sql,
    Jsonnet,
    Json,
}
impl Language {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Sql => "sql",
            Self::Json => "json",
            Self::Jsonnet => "jsonnet",
        }
    }
}

fn highlight<T: std::fmt::Display>(
    snippet: T,
    language: Language,
    args: &Flags,
) -> Result<(), Error> {
    if args.execute {
        // TODO: Get `bat` to output on stderr. This currently cannot be configured with the
        // PrettyPrinter
        return Ok(());
    }
    let is_tty = std::io::stdout().is_terminal();
    if is_tty {
        let mut printer = bat::PrettyPrinter::new();
        if let Some(theme) = &args.theme {
            printer.theme(theme);
        }
        let sql = std::io::Cursor::new(snippet.to_string());
        printer
            .input(bat::Input::from_reader(sql).name(format!("queries.{}", language.as_str())))
            .language(language.as_str())
            .header(true);

        printer.print()?;
    } else {
        eprintln!("{}", snippet);
    }
    eprintln!();
    Ok(())
}

#[tokio::main]
async fn main() -> miette::Result<()> {
    Ok(main_impl().await?)
}

async fn process_one(
    args: &Flags,
    client: &Option<clickhouse_client::HttpClient>,
) -> Result<(), Error> {
    let start = std::time::Instant::now();
    let display_format = args.display_format.clone().unwrap_or_else(|| {
        vec![if args.from_sql {
            Language::Jsonnet
        } else {
            Language::Sql
        }]
    });

    let filename = args.input.filename();
    let input = args.input.clone().contents()?;
    if args.from_sql {
        info!("Converting SQL file {}", filename);
        let queries = Queries::from_sql(&input)?;
        let has = |l| display_format.iter().any(|l2| l2 == &l);
        let sql = queries.to_sql(args.compact);
        if has(Language::Sql) {
            highlight(&sql, Language::Sql, args)?;
        }
        if has(Language::Jsonnet) {
            let jsonnet = queries.as_jsonnet();
            highlight(jsonnet, Language::Jsonnet, args)?;
        }
        if args.diff && input != sql {
            eprintln!("{}", pretty_assertions::StrComparison::new(&input, &sql));
        }
    } else {
        let contents = sqlsonnet::jsonnet::import_utils() + &input;
        info!("Converting Jsonnet file {} to SQL", filename);

        let mut resolver = sqlsonnet::jsonnet::FsResolver::current_dir();
        if let Some(jpath) = args.jpath.clone() {
            for path in jpath {
                resolver.add(path);
            }
        }

        // TODO: Support passing a single query.
        let queries = Queries::from_jsonnet(
            &contents,
            sqlsonnet::jsonnet::Options::new(
                resolver,
                concat!(env!("CARGO_BIN_NAME"), " ", env!("CARGO_PKG_VERSION")),
            ),
        )?;

        let has = |l| display_format.iter().any(|l2| l2 == &l);
        // Display queries
        debug!("{:#?}", queries);
        if has(Language::Jsonnet) {
            highlight(&contents, Language::Jsonnet, args)?;
        }
        if has(Language::Sql) {
            highlight(queries.to_sql(args.compact), Language::Sql, args)?;
        }
        if let Some(client) = client {
            info!("Executing query on Clickhouse");
            for query in queries {
                let resp = client
                    .send_query(&clickhouse_client::ClickhouseQuery {
                        query: query.to_sql(false),
                        params: BTreeMap::from([(
                            "default_format".into(),
                            args.execute_format.clone(),
                        )]),
                        compression: clickhouse_client::Compression::Zstd,
                    })
                    .await?
                    .text()
                    .await
                    .map_err(clickhouse_client::Error::from)?;
                println!("{}", resp);
            }
        }
    }

    info!(elapsed=?start.elapsed(), "Done");
    Ok(())
}

async fn main_impl() -> Result<(), Error> {
    sqlsonnet::setup_logging();
    let mut args = Flags::parse();
    if !args.execute {
        args.clickhouse_url = None;
    }

    let assets = bat::assets::HighlightingAssets::from_binary();
    let theme = assets.get_theme(
        args.theme
            .as_deref()
            .unwrap_or_else(|| bat::assets::HighlightingAssets::default_theme()),
    );
    let highlighter = miette::highlighters::SyntectHighlighter::new(
        assets.get_syntax_set().unwrap().clone(),
        theme.clone(),
        false,
    );
    miette::set_hook(Box::new(move |_| {
        Box::new(
            miette::MietteHandlerOpts::new()
                .context_lines(10)
                .with_syntax_highlighting(highlighter.clone())
                .build(),
        )
    }))?;

    let client = args
        .clickhouse_url
        .clone()
        .map(|url| clickhouse_client::HttpClient::new(url, true /* auto-decompress */));

    if args.watch && args.input.is_file() {
        // Watch mode
        let (tx, rx) = std::sync::mpsc::channel();
        let mut debouncer =
            notify_debouncer_mini::new_debouncer(std::time::Duration::from_millis(500), tx)?;
        debouncer.watcher().watch(
            std::path::Path::new(args.input.filename()),
            notify_debouncer_mini::notify::RecursiveMode::NonRecursive,
        )?;
        loop {
            if let Err(e) = process_one(&args, &client).await {
                error!("{}", e);
            }
            info!("Watching {} for changes", args.input.filename());
            rx.recv().unwrap()?;
        }
    } else {
        process_one(&args, &client).await?;
    }

    Ok(())
}
