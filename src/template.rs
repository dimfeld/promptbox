use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use error_stack::{Report, ResultExt};
use serde::Deserialize;

use crate::{error::Error, model::ModelOptionsInput};

#[derive(Deserialize, Debug, Default, Copy, Clone)]
#[serde(rename_all = "lowercase")]
pub enum OptionType {
    #[default]
    String,
    Number,
    Integer,
    Bool,
    File,
}

#[derive(Deserialize, Debug)]
pub struct PromptOption {
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub array: bool,
    #[serde(rename = "type", default)]
    pub option_type: OptionType,
    #[serde(default)]
    pub optional: bool,
}

#[derive(Deserialize, Debug)]
pub struct PromptTemplate {
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub model: ModelOptionsInput,

    #[serde(default)]
    pub options: HashMap<String, PromptOption>,

    pub template: Option<String>,
    pub template_path: Option<PathBuf>,
}

#[derive(Debug)]
pub struct ParsedTemplate {
    pub name: String,
    pub input: PromptTemplate,
    pub path: PathBuf,
    pub template: String,
}

impl ParsedTemplate {
    /// Try to load a template from a file. If the file does not exist, returns `Ok(None)`.
    pub fn from_file(name: &str, path: &Path) -> Result<Option<Self>, Report<Error>> {
        let Ok(contents) = std::fs::read_to_string(path) else {
            return Ok(None);
        };

        let mut prompt_template: PromptTemplate = toml::from_str(&contents)
            .change_context(Error::ParseTemplate)
            .attach_printable_lazy(|| path.display().to_string())?;

        // At some point we should support partials here, but it still needs some design since we
        // want to allow templates to reference partials in upper directories. For now, we just
        // do a String.
        let (template_path, template_result) = if let Some(t) = prompt_template.template.take() {
            // Template is embedded in the file
            (path.to_path_buf(), t)
        } else {
            // Load it from the specified path
            let relative_template_path = prompt_template
                .template_path
                .as_ref()
                .ok_or(Error::EmptyTemplate)?;
            let template_path = path
                .parent()
                .ok_or(Error::EmptyTemplate)?
                .join(relative_template_path);

            let template_contents = std::fs::read_to_string(&template_path)
                .change_context(Error::TemplateContentsNotFound)
                .attach_printable_lazy(|| template_path.display().to_string())?;
            (template_path, template_contents)
        };

        Ok(Some(ParsedTemplate {
            name: name.to_string(),
            input: prompt_template,
            path: template_path,
            template: template_result,
        }))
    }
}
