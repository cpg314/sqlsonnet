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
}

#[derive(thiserror::Error, Diagnostic, Debug)]
#[error("Failed to compile Jsonnet: {reason}")]
pub struct JsonnetError {
    pub reason: String,
    // TODO: Mark the language as JSON
    #[source_code]
    pub src: String,
    #[label]
    pub span: Option<miette::SourceSpan>,
}

#[derive(thiserror::Error, Diagnostic, Debug)]
#[error("Failed to deserialize JSON: {reason}")]
pub struct JsonError {
    pub reason: String,
    // TODO: Mark the language as JSON
    #[source_code]
    pub src: String,
    #[label]
    pub span: miette::SourceOffset,
}

#[derive(thiserror::Error, Diagnostic, Debug)]
#[error("Failed to parse SQL: {reason}")]
pub struct SQLParseError {
    pub reason: String,
    // TODO: Mark the language as SQL
    #[source_code]
    pub src: String,
    #[label]
    pub span: miette::SourceOffset,
}
