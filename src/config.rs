use std::path::{Path, PathBuf};

use error_stack::{Report, ResultExt};
use serde::Deserialize;

use crate::{
    error::Error,
    global_config::global_config_dirs,
    model::{ModelOptions, ModelOptionsInput},
    template::ParsedTemplate,
};

fn default_template_dirs() -> Vec<PathBuf> {
    vec![PathBuf::from(".")]
}

#[derive(Deserialize, Debug, Default)]
pub struct ConfigInput {
    /// One or more globs that define where to look for templates.
    /// Defaults to ./promptbox, or ./ if the config file is in ./promptbox
    #[serde(default = "default_template_dirs")]
    pub templates: Vec<PathBuf>,
    /// Stop recursing through parent directories if a config file is found with `top_level = true`
    #[serde(default)]
    pub top_level: bool,
    /// Do not use the global config if this is `false`.
    pub use_global_config: Option<bool>,
    /// Default model options to use for any prompts that don't override them.
    pub model: Option<ModelOptionsInput>,
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
            for global_config_dir in global_config_dirs() {
                if let Some(new_config) = ConfigInput::from_dir(&global_config_dir)? {
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
            let template_path = template_dir.join(format!("{}.pb.toml", name));
            match ParsedTemplate::from_file(name, &template_path) {
                Ok(Some(template)) => return Ok(template),
                // template was not found in this directory, but that's ok.
                Ok(None) => (),
                Err(error) => return Err(error),
            }
        }

        Err(Report::from(Error::TemplateNotFound))
    }
}

impl ConfigInput {
    fn from_dir(dir: &Path) -> Result<Option<Self>, Report<Error>> {
        let mut config_iter = ["promptbox.toml", "promptbox/promptbox.toml"]
            .into_iter()
            .filter_map(|p| {
                let config_path = dir.join(p);
                let contents = std::fs::read_to_string(&config_path).ok()?;
                Some((config_path, contents))
            });

        let Some((config_path, contents)) = config_iter.next() else {
            return Ok(None);
        };

        let mut new_config: ConfigInput = toml::from_str(&contents)
            .change_context(Error::ParseConfig)
            .attach_printable_lazy(|| config_path.display().to_string())?;

        let base_dir = config_path.parent().expect("path had no directory");
        new_config.resolve_template_dirs(base_dir);
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

#[cfg(test)]
mod tests {

    #[test]
    #[ignore]
    fn resolve_hierarchy() {}

    #[test]
    #[ignore]
    fn config_in_subdir() {}

    #[test]
    #[ignore]
    fn intermediate_without_config() {}

    #[test]
    #[ignore]
    fn malformed() {}

    #[test]
    #[ignore]
    fn stop_at_toplevel_setting() {}
}
