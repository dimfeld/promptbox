use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use error_stack::{Report, ResultExt};
use serde::Deserialize;

use crate::{
    error::Error,
    global_config::global_config_dirs,
    hosts::{HostDefinition, HostDefinitionInput},
    model::{ModelOptions, ModelOptionsInput},
    option::overwrite_option_from_option,
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
    /// Custom hosts that can serve model requests.
    #[serde(default)]
    pub host: HashMap<String, HostDefinitionInput>,
    /// The default model host to use. If absent, ollama is the default.
    /// GPT 3.5/4 models will always use OpenAI as the default if not explicitly set otherwise.
    pub default_host: Option<String>,
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
                let top_level = new_config.top_level;
                config.merge(new_config);
                if top_level {
                    break;
                }
            }

            if !current_dir.pop() {
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

        let mut hosts = HostDefinition::builtin();

        for (k, host_input) in config.host {
            if let Some(host) = hosts.get_mut(&k) {
                host.merge_from_input(&host_input);
            } else {
                let host = HostDefinition::try_from(host_input)
                    .attach_printable_lazy(|| format!("Host {k}"))
                    .change_context(Error::ParseConfig)?;
                hosts.insert(k, host);
            }
        }

        Ok(Self {
            template_dirs: config.templates,
            model: ModelOptions::new(
                config.model.unwrap_or_default(),
                hosts,
                config
                    .default_host
                    .unwrap_or_else(|| HostDefinition::default_host().to_string()),
            ),
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
    /// Try to load a ConfigInput from a directory or the `promptbox` sudirectory.
    fn from_dir(dir: &Path) -> Result<Option<Self>, Report<Error>> {
        let mut config_iter = ["promptbox.toml", "promptbox/promptbox.toml"]
            .into_iter()
            .filter_map(|p| {
                let config_path = dir.join(p);
                let contents = std::fs::read_to_string(&config_path).ok()?;
                Some((config_path, contents))
            });

        let Some((config_path, contents)) = config_iter.next() else {
            // If there is a directory named promptbox, but without a config file, use that.
            let promptbox_dir = dir.join("promptbox");
            if promptbox_dir.is_dir() {
                return Ok(Some(ConfigInput {
                    templates: vec![promptbox_dir],
                    ..Default::default()
                }));
            }

            return Ok(None);
        };

        let mut new_config: ConfigInput = toml::from_str(&contents)
            .change_context(Error::ParseConfig)
            .attach_printable_lazy(|| config_path.display().to_string())?;

        let base_dir = config_path.parent().expect("path had no directory");
        new_config.resolve_template_dirs(base_dir);
        Ok(Some(new_config))
    }

    /// Convert the template directory references to absolute paths
    fn resolve_template_dirs(&mut self, base_dir: &Path) {
        for template in self.templates.iter_mut() {
            if template.is_relative() {
                if let Ok(full_path) = std::fs::canonicalize(base_dir.join(&template)) {
                    *template = full_path;
                }
            }
        }
    }

    /// Merge in another ConfigInput, using only values which are not yet configured in `self`.
    fn merge(&mut self, other: ConfigInput) {
        self.templates.extend(other.templates);

        overwrite_option_from_option(&mut self.use_global_config, &other.use_global_config);

        if let Some(other_model) = other.model {
            if let Some(model) = self.model.as_mut() {
                model.merge_defaults(&other_model);
            } else {
                self.model = Some(other_model);
            }
        }

        for (key, other_host) in other.host {
            if let Some(host) = self.host.get_mut(&key) {
                host.merge_from_input(&other_host);
            } else {
                self.host.insert(key, other_host);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{base_dir, BASE_DIR};

    #[test]
    fn config_in_subdir() {
        let config = Config::from_directory(base_dir("config_in_subdir")).expect("loading config");
        let expected_dirs = vec![
            base_dir("config_in_subdir/promptbox"),
            PathBuf::from(BASE_DIR),
        ];
        assert_eq!(config.template_dirs, expected_dirs);
    }

    #[test]
    fn intermediate_without_config() {
        let config =
            Config::from_directory(base_dir("intermediate_without_config/leaf_dir_with_config"))
                .expect("loading config");
        let expected_dirs = vec![
            base_dir("intermediate_without_config/leaf_dir_with_config"),
            PathBuf::from(BASE_DIR),
        ];
        assert_eq!(config.template_dirs, expected_dirs);
    }

    #[test]
    fn malformed() {
        let err = Config::from_directory(base_dir("malformed_config"))
            .expect_err("loading config should fail");
        assert!(matches!(err.current_context(), Error::ParseConfig));
    }

    #[test]
    fn stop_at_toplevel_setting() {
        let config = Config::from_directory(base_dir("toplevel_config")).expect("loading config");
        let expected_dirs = vec![base_dir("toplevel_config")];
        assert_eq!(config.template_dirs, expected_dirs);
        assert_eq!(config.model.temperature, 1.2);
        assert_eq!(
            config.model.top_p, None,
            "Should not read values from the parent directory"
        );
    }
}
