use std::io::IsTerminal;
use std::path::Path;
use std::str::FromStr;

use clap::Parser;
use tracing::*;

use sqlsonnet::Error;
use sqlsonnet::Queries;

lazy_static::lazy_static! {
    static ref THEMES: Vec<String> =
        bat::assets::HighlightingAssets::from_binary().themes().map(String::from).collect();
}

#[derive(Parser)]
#[clap(version)]
struct Flags {
    /// Color theme for syntax highlighting
    #[clap(long, env = "SQLSONNET_THEME",
           value_parser=clap::builder::PossibleValuesParser::new(THEMES.iter().map(|s| s.as_str())))]
    theme: Option<String>,
    #[clap(subcommand)]
    command: Command,
    /// Compact SQL representation
    #[clap(long, short)]
    compact: bool,
}

#[derive(Parser)]
enum Command {
    /// Convert SQL to Jsonnet
    FromSql {
        /// Input file (path or - for stdin).
        input: Input,
        #[clap(long, value_delimiter = ',', default_value = "jsonnet")]
        /// Display the converted Jsonnet output and/or the SQL roundtrip
        display_format: Vec<Language>,
        /// Convert back to SQL and print the differences with the original, if any
        #[clap(long)]
        diff: bool,
    },
    /// Convert Jsonnet to SQL
    ToSql {
        /// Input file (path or - for stdin).
        input: Input,
        #[clap(long, value_delimiter = ',', default_value = "sql")]
        /// Display the converted SQL, the intermediary Json, or the original Jsonnet.
        display_format: Vec<Language>,
    },
}

#[derive(Clone)]
struct Input(clap_stdin::FileOrStdin);
impl FromStr for Input {
    type Err = <clap_stdin::FileOrStdin as FromStr>::Err;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        clap_stdin::FileOrStdin::from_str(s).map(Self)
    }
}

impl Input {
    fn contents(&self) -> Result<String, clap_stdin::StdinError> {
        self.0.clone().contents()
    }
    fn filename(&self) -> String {
        match &self.0.source {
            clap_stdin::Source::Stdin => "<stdin>".into(),
            clap_stdin::Source::Arg(s) => s.clone(),
        }
    }
    fn import_paths(&self) -> sqlsonnet::ImportPaths {
        match &self.0.source {
            clap_stdin::Source::Stdin => Default::default(),
            clap_stdin::Source::Arg(s) => Path::new(s)
                .parent()
                .map(sqlsonnet::ImportPaths::from)
                .unwrap_or_default(),
        }
    }
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
        println!("{}", snippet);
    }
    println!();
    Ok(())
}

fn main() -> miette::Result<()> {
    Ok(main_impl()?)
}

fn main_impl() -> Result<(), Error> {
    let start = std::time::Instant::now();
    sqlsonnet::setup_logging();
    let args = Flags::parse();

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

    match &args.command {
        Command::ToSql {
            input,
            display_format,
        } => {
            let filename = input.filename();
            let contents = input.contents()?;
            info!("Converting Jsonnet file {} to SQL", filename);

            let queries = Queries::from_jsonnet(&contents, input.import_paths())?;

            let has = |l| display_format.iter().any(|l2| l2 == &l);
            // Display queries
            debug!("{:#?}", queries);
            if has(Language::Jsonnet) {
                highlight(&contents, Language::Jsonnet, &args)?;
            }
            if has(Language::Sql) {
                highlight(queries.to_sql(args.compact), Language::Sql, &args)?;
            }
        }
        Command::FromSql {
            input,
            display_format,
            diff,
        } => {
            info!("Converting SQL file {}", input.filename());
            let input = input.contents()?;
            let queries = Queries::from_sql(&input)?;
            let has = |l| display_format.iter().any(|l2| l2 == &l);
            let sql = queries.to_sql(args.compact);
            if has(Language::Sql) {
                highlight(&sql, Language::Sql, &args)?;
            }
            if has(Language::Jsonnet) {
                let jsonnet = queries.as_jsonnet();
                highlight(jsonnet, Language::Jsonnet, &args)?;
            }
            if *diff && input != sql {
                println!("{}", pretty_assertions::StrComparison::new(&input, &sql));
            }
        }
    }

    info!(elapsed=?start.elapsed(), "Done");
    Ok(())
}
