use std::path::{Path, PathBuf};

use error_stack::{Report, ResultExt};
use serde::Deserialize;

use crate::{
    error::Error,
    model::{ModelOptions, ModelOptionsInput},
    template::ParsedTemplate,
};

fn default_templates_dir() -> Vec<PathBuf> {
    vec![PathBuf::from(".")]
}

#[derive(Deserialize, Debug, Default)]
pub struct ConfigInput {
    /// Directories in which to look for templates.
    #[serde(default = "default_templates_dir")]
    pub templates: Vec<PathBuf>,
    /// Default model options to use for any prompts that don't override them.
    pub model: Option<ModelOptionsInput>,
    /// Stop recursing through parent directories if a config file is found with `top_level = true`
    #[serde(default)]
    pub top_level: bool,
    /// Do not use the global config if this is `false`.
    pub use_global_config: Option<bool>,
}

#[derive(Debug, Default)]
pub struct Config {
    pub template_dirs: Vec<PathBuf>,
    pub model: ModelOptions,
}

impl Config {
    /// Create a [Config], recursing from the directory given up through the parent directories.
    pub fn from_directory(start_dir: PathBuf) -> Result<Self, Report<Error>> {
        let mut config = ConfigInput::default();

        let mut current_dir = start_dir;
        loop {
            if let Some(new_config) = ConfigInput::from_dir(&current_dir)? {
                config.merge(new_config);
            }

            if config.top_level || !current_dir.pop() {
                break;
            }
        }

        if config.use_global_config.unwrap_or(true) {
            let global_config_paths = [
                dirs::config_dir(),
                dirs::home_dir().map(|p| p.join(".config")),
            ];

            for global_config_dir in global_config_paths.into_iter().flatten() {
                let global_config = global_config_dir.join("promptbox");
                if let Some(new_config) = ConfigInput::from_dir(&global_config)? {
                    config.merge(new_config);
                }
            }
        }

        Ok(Self {
            template_dirs: config.templates,
            model: ModelOptions::from(config.model.unwrap_or_default()),
        })
    }

    pub fn find_template(&self, name: &str) -> Result<ParsedTemplate, Report<Error>> {
        for template_dir in &self.template_dirs {
            for potential_suffix in ["mp.toml", "toml"] {
                let template_path = template_dir.join(format!("{}.{}", name, potential_suffix));
                match ParsedTemplate::from_file(&template_path) {
                    Ok(Some(template)) => return Ok(template),
                    // template was not found in this directory, but that's ok.
                    Ok(None) => (),
                    Err(error) => return Err(error),
                }
            }
        }

        Err(Report::from(Error::TemplateNotFound))
    }
}

impl ConfigInput {
    fn from_dir(dir: &Path) -> Result<Option<Self>, Report<Error>> {
        let config_path = dir.join("promptbox.toml");
        let Ok(contents) = std::fs::read_to_string(&config_path) else {
            return Ok(None);
        };

        let mut new_config: ConfigInput = toml::from_str(&contents)
            .change_context(Error::ParseConfig)
            .attach_printable_lazy(|| config_path.display().to_string())?;
        new_config.resolve_template_dirs(dir);
        Ok(Some(new_config))
    }

    fn resolve_template_dirs(&mut self, base_dir: &Path) {
        for template in self.templates.iter_mut() {
            if template.is_relative() {
                if let Ok(full_path) = std::fs::canonicalize(base_dir.join(&template)) {
                    *template = full_path;
                }
            }
        }
    }

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
