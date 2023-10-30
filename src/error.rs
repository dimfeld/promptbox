use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Error parsing configuration file")]
    ParseConfig,
}
