use crate::model::ModelOptions;

pub struct Config {
    pub templates: Vec<String>,
    /// Default model options to use for any prompts that don't override them.
    pub model: Option<ModelOptions>,
}
