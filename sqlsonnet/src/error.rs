use miette::Diagnostic;

#[derive(thiserror::Error, Diagnostic, Debug)]
pub enum Error {
    #[error("Failed to read input")]
    Input(#[from] clap_stdin::StdinError),
    #[error(transparent)]
    #[diagnostic(transparent)]
    Jsonnet(#[from] JsonnetError),
    #[error(transparent)]
    #[diagnostic(transparent)]
    Json(#[from] JsonError),
    #[error(transparent)]
    #[diagnostic(transparent)]
    SqlParse(#[from] SQLParseError),
    #[error("Failed to highlight SQL")]
    Bad(#[from] bat::error::Error),
    #[error(transparent)]
    Miette(#[from] miette::InstallError),
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
