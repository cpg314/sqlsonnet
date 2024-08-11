use itertools::Itertools;
use miette::{Diagnostic, SourceCode};

/// Errors
#[derive(thiserror::Error, Diagnostic, Debug)]
pub enum Error {
    #[error(transparent)]
    #[diagnostic(transparent)]
    Jsonnet(#[from] JsonnetError),
    #[error(transparent)]
    #[diagnostic(transparent)]
    Json(#[from] JsonError),
    #[error(transparent)]
    #[diagnostic(transparent)]
    SqlParse(#[from] SQLParseError),
    #[error("Invalid jsonnet value (must cast to finite f64)")]
    InvalidValue(#[from] jrsonnet_evaluator::val::ConvertNumValueError),
}

#[derive(thiserror::Error, Diagnostic, Debug)]
#[error("Failed to compile Jsonnet: {reason}")]
pub struct JsonnetError {
    pub reason: String,
    #[source_code]
    pub src: miette::NamedSource<String>,
    #[label]
    pub span: Option<miette::SourceSpan>,
}

impl JsonnetError {
    pub fn from(src: &str, error: jrsonnet_evaluator::Error) -> Self {
        let reason = error.to_string();
        if let jrsonnet_evaluator::error::ErrorKind::ImportSyntaxError { error, path } =
            error.error()
        {
            Self {
                reason,
                src: miette::NamedSource::new(path.source_path().to_string(), path.code().into()),
                span: Some(miette::SourceSpan::new(error.location.offset.into(), 1)),
            }
        } else {
            Self {
                reason,
                src: miette::NamedSource::new("source.jsonnet", src.into()),
                span: None,
            }
        }
    }
}

#[derive(thiserror::Error, Diagnostic, Debug)]
#[error("Failed to interpret query from JSON: {reason}")]
pub struct JsonError {
    pub reason: String,
    #[source_code]
    pub src: miette::NamedSource<String>,
    #[label]
    pub span: miette::SourceOffset,
}
impl JsonError {
    pub fn from(json: &str, e: serde_json::Error) -> Self {
        Self {
            reason: e.to_string(),
            span: miette::SourceOffset::from_location(json, e.line(), e.column()),
            src: miette::NamedSource::new("source.json", json.into()),
        }
    }
}
#[derive(thiserror::Error, Diagnostic, Debug)]
#[error("Failed to parse SQL: {reason}")]
pub struct SQLParseError {
    pub reason: String,
    #[source_code]
    pub src: miette::NamedSource<String>,
    #[label]
    pub span: miette::SourceOffset,
}

/// Converted errors with message, source code, and location.
#[derive(serde::Serialize)]
pub struct FormattedError {
    pub message: String,
    pub code: Option<String>,
    /// Line and column
    pub location: Option<[usize; 2]>,
}
impl From<String> for FormattedError {
    fn from(source: String) -> Self {
        Self {
            message: source,
            code: None,
            location: None,
        }
    }
}

impl Error {
    pub fn formatted(self) -> FormattedError {
        FormattedError::from(self)
    }
}
impl From<Error> for FormattedError {
    fn from(source: Error) -> Self {
        match &source {
            Error::Jsonnet(_) => {
                Self {
                    message: source.to_string(),
                    code: None,
                    location: if let (Some(source_code), Some(labels)) =
                        (source.source_code(), source.labels())
                    {
                        labels
                            .filter_map(|l| source_code.read_span(l.inner(), 0, 0).ok())
                            // Subtract 1 for the initial line
                            .map(|sc| [sc.line() - 1, sc.column()])
                            .next()
                    } else {
                        None
                    },
                }
            }
            Error::Json(json_source) => Self {
                message: source.to_string(),
                code: json_source
                    .src
                    .read_span(&miette::SourceSpan::new(json_source.span, 1), 2, 2)
                    .ok()
                    .and_then(|contents| String::from_utf8(contents.data().into()).ok())
                    .map(|code| {
                        let indent = code
                            .lines()
                            .filter(|l| !l.is_empty())
                            .map(|l| l.chars().take_while(|c| c.is_whitespace()).count())
                            .min()
                            .unwrap_or_default();
                        let indent: String = " ".repeat(indent);
                        code.lines()
                            .map(|l| l.strip_prefix(&indent).unwrap_or(l))
                            .join("\n")
                    }),
                location: None,
            },

            _ => source.into(),
        }
    }
}
