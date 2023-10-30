use serde::Deserialize;

use crate::model::ModelOptionsInput;

#[derive(Deserialize, Debug)]
#[serde(rename_all = "lowercase")]
pub enum OptionType {
    String,
    Number,
    Integer,
    Bool,
}

#[derive(Deserialize, Debug)]
pub struct PromptOption {
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub array: bool,
    #[serde(rename = "type")]
    pub option_type: OptionType,
    #[serde(default)]
    pub required: bool,
}

#[derive(Deserialize, Debug)]
pub struct PromptTemplate {
    pub name: String,
    pub description: String,
    pub model: ModelOptionsInput,

    #[serde(default)]
    pub options: Vec<PromptOption>,

    pub template: Option<String>,
    pub template_path: Option<String>,
}
