use std::path::PathBuf;

use error_stack::{Report, ResultExt};
use serde::Deserialize;

use crate::{
    error::Error,
    model::{ModelOptions, ModelOptionsInput},
};

#[derive(Deserialize, Debug, Default)]
pub struct ConfigInput {
    #[serde(default)]
    pub templates: Vec<String>,
    /// Default model options to use for any prompts that don't override them.
    pub model: Option<ModelOptionsInput>,
}

#[derive(Debug, Default)]
pub struct Config {
    pub templates: Vec<String>,
    pub model: ModelOptions,
}

impl Config {
    /// Create a [Config], recursing from the directory given up through the parent directories.
    pub fn from_directory(start_dir: PathBuf) -> Result<Self, Report<Error>> {
        let mut config = ConfigInput::default();

        let mut current_dir = start_dir.clone();
        loop {
            let config_path = current_dir.join("movableprompt.toml");
            if let Ok(c) = std::fs::read_to_string(&config_path) {
                let new_config: ConfigInput = toml::from_str(&c)
                    .change_context(Error::ParseConfig)
                    .attach_printable_lazy(|| config_path.display().to_string())?;
                config.merge(new_config);
            }

            if !current_dir.pop() {
                break;
            }
        }

        let global_config = dirs::config_dir()
            .unwrap()
            .join("movableprompt")
            .join("movableprompt.toml");
        if let Ok(c) = std::fs::read_to_string(&global_config) {
            let new_config: ConfigInput = toml::from_str(&c)
                .change_context(Error::ParseConfig)
                .attach_printable_lazy(|| global_config.display().to_string())?;
            config.merge(new_config);
        }

        Ok(Self {
            templates: config.templates,
            model: ModelOptions::from(config.model.unwrap_or_default()),
        })
    }
}

impl ConfigInput {
    fn merge(&mut self, other: ConfigInput) {
        self.templates.extend(other.templates);

        if let Some(other_model) = other.model {
            if let Some(model) = self.model.as_mut() {
                model.merge_defaults(&other_model);
            } else {
                self.model = Some(other_model);
            }
        }
    }
}

pub fn merge_option<T: Clone>(a: &mut Option<T>, b: &Option<T>) {
    if a.is_none() && b.is_some() {
        *a = b.clone();
    }
}
