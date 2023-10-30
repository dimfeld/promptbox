use serde::Deserialize;

use crate::model::ModelOptions;

#[derive(Deserialize, Debug)]
pub struct PromptOption {
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub required: bool,
}

#[derive(Deserialize, Debug)]
pub struct PromptTemplate {
    pub name: String,
    pub decription: String,
    pub model: ModelOptions,

    #[serde(default)]
    pub options: Vec<PromptOption>,

    pub template: String,
}
