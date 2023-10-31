use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Error reading configuration file")]
    ParseConfig,
    #[error("Error reading template")]
    ParseTemplate,
    #[error("Template not found")]
    TemplateNotFound,
    #[error("Template contents not found")]
    TemplateContentsNotFound,
    #[error("This template is missing template and template_path")]
    EmptyTemplate,
    #[error("Failed to parse arguments")]
    ArgParseFailure,
}
