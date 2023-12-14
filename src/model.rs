use std::{collections::HashMap, str::FromStr};

use error_stack::{Report, ResultExt};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    args::GlobalRunArgs,
    context::{ContextOptions, ContextOptionsInput},
    error::Error,
    hosts::{HostDefinition, ModelHost},
    option::{overwrite_from_option, overwrite_option_from_option, update_if_none},
};

#[derive(Debug, Clone)]
pub struct ModelOptions {
    pub model: ModelSpec,
    pub openai_key: Option<String>,
    pub temperature: f32,
    pub format: Option<OutputFormat>,
    pub top_k: Option<u32>,
    pub top_p: Option<f32>,
    pub frequency_penalty: Option<f32>,
    pub presence_penalty: Option<f32>,
    pub stop: Vec<String>,
    pub max_tokens: Option<u32>,
    /// Alias of short model names to full names, useful for ollama, for example
    pub alias: HashMap<String, ModelSpec>,

    /// Hosts parsed from the configuration
    pub host: HashMap<String, HostDefinition>,
    /// The default host to use for non-OpenAI models, when no other host is specified.
    pub default_host: String,

    pub context: ContextOptions,
}

const DEFAULT_MODEL: &str = "gpt-3.5-turbo";
const DEFAULT_TEMPERATURE: f32 = 0.0;

impl Default for ModelOptions {
    fn default() -> Self {
        Self {
            model: ModelSpec::default(),
            openai_key: None,
            temperature: DEFAULT_TEMPERATURE,
            format: None,
            top_k: None,
            top_p: None,
            frequency_penalty: None,
            presence_penalty: None,
            stop: Vec::new(),
            max_tokens: None,
            context: ContextOptions::default(),
            alias: HashMap::new(),
            host: HostDefinition::builtin(),
            default_host: HostDefinition::default_host().to_string().to_string(),
        }
    }
}

impl ModelOptions {
    pub fn new(
        value: ModelOptionsInput,
        host: HashMap<String, HostDefinition>,
        default_host: String,
    ) -> Self {
        Self {
            model: value.model.unwrap_or_default(),
            // For security, don't allow setting openAI key in normal config or template files.
            openai_key: None,
            temperature: value.temperature.unwrap_or(DEFAULT_TEMPERATURE),
            format: value.format,
            top_p: value.top_p,
            top_k: value.top_k,
            frequency_penalty: value.frequency_penalty,
            presence_penalty: value.presence_penalty,
            stop: value.stop.unwrap_or_default(),
            max_tokens: value.max_tokens,
            alias: value.alias,
            context: value.context.into(),
            host,
            default_host,
        }
    }

    pub fn update_from_args(&mut self, args: &GlobalRunArgs) {
        let model_spec = match (&args.model, &args.model_host) {
            (Some(model), host) => Some(ModelSpec::Full {
                model: model.clone(),
                host: host.clone(),
            }),
            (_, _) => None,
        };

        overwrite_from_option(&mut self.model, &model_spec);
        overwrite_from_option(&mut self.temperature, &args.temperature);
        overwrite_option_from_option(&mut self.format, &args.format);
        overwrite_from_option(&mut self.context.keep, &args.overflow_keep);
        overwrite_option_from_option(&mut self.context.limit, &args.context_limit);
        overwrite_from_option(
            &mut self.context.reserve_output,
            &args.reserve_output_context,
        );

        // Always overwrite this since there's no other way to set the key.
        self.openai_key = args.openai_key.clone();
    }

    pub fn full_model_spec(&self) -> ModelSpec {
        self.alias
            .get(self.model.model_name())
            .map(|alias| self.model.merge_with_alias_spec(alias))
            .unwrap_or_else(|| self.model.clone())
    }

    pub fn api_host(&self) -> Result<Box<dyn ModelHost>, Error> {
        let model_spec = self.full_model_spec();
        let host_name = match model_spec.host_name() {
            Some(host) => host,
            None => {
                let model = model_spec.model_name();
                if model.starts_with("gpt-4") || model.starts_with("gpt-3.5-") {
                    "openai"
                } else if model == "lm-studio" {
                    "lm-studio"
                } else {
                    &self.default_host
                }
            }
        };

        self.host
            .get(host_name)
            .ok_or(Error::UnknownModelHost(host_name.to_string()))
            .map(|host| host.into_model_host())
    }

    pub fn update_from_model_input(&mut self, other: &ModelOptionsInput) {
        overwrite_from_option(&mut self.model, &other.model);
        overwrite_from_option(&mut self.temperature, &other.temperature);
        overwrite_option_from_option(&mut self.format, &other.format);
        overwrite_option_from_option(&mut self.top_p, &other.top_p);
        overwrite_option_from_option(&mut self.top_k, &other.top_k);
        overwrite_option_from_option(&mut self.frequency_penalty, &other.frequency_penalty);
        overwrite_option_from_option(&mut self.presence_penalty, &other.presence_penalty);
        overwrite_from_option(&mut self.stop, &other.stop);
        overwrite_option_from_option(&mut self.max_tokens, &other.max_tokens);

        for (key, value) in &other.alias {
            if !self.alias.contains_key(key) {
                self.alias.insert(key.clone(), value.clone());
            }
        }
    }

    /// Get the input context size limit for a model.
    /// The returned value is the total context size minus `self.context.reserve_output`.
    /// This may do a network request for Ollama models.
    pub fn context_limit(&self) -> Result<Option<usize>, Report<Error>> {
        let model = self.full_model_spec();
        let model_name = model.model_name();

        let comms = self.api_host()?;
        let limit = comms
            .model_context_limit(model_name)
            .change_context(Error::ContextLimit)?;

        let Some(limit) = limit else {
            return Ok(None);
        };

        let limit = std::cmp::min(limit, self.context.limit.unwrap_or(usize::MAX));

        if limit <= (self.context.reserve_output + 1) {
            return Err(Report::new(Error::ContextLimit)).attach_printable(
                "The context size does not leave enough space for the reserved output size.",
            );
        }

        Ok(Some(limit - self.context.reserve_output))
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum OutputFormat {
    JSON,
}

impl FromStr for OutputFormat {
    type Err = Report<crate::error::Error>;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "json" => Ok(Self::JSON),
            _ => Err(crate::error::Error::ArgParseFailure).attach_printable_lazy(|| s.to_string()),
        }
    }
}

#[derive(Deserialize, Debug, Eq, Clone)]
#[serde(untagged)]
pub enum ModelSpec {
    Plain(String),
    Full { model: String, host: Option<String> },
}

impl ModelSpec {
    pub fn model_name(&self) -> &str {
        match self {
            Self::Plain(model) => model,
            Self::Full { model, .. } => model,
        }
    }

    pub fn host_name(&self) -> Option<&str> {
        match self {
            Self::Plain(_) => None,
            Self::Full { host, .. } => host.as_deref(),
        }
    }

    /// Given a alias spec, return a new spec that uses the model name from the alias spec
    /// gives precedence to self for the other fields.
    pub fn merge_with_alias_spec(&self, alias_spec: &ModelSpec) -> Self {
        match (self, alias_spec) {
            (ModelSpec::Plain(_), alias) => alias.clone(),
            (ModelSpec::Full { host, .. }, ModelSpec::Plain(real_model)) => ModelSpec::Full {
                model: real_model.clone(),
                host: host.clone(),
            },
            (
                ModelSpec::Full {
                    host: self_host, ..
                },
                ModelSpec::Full {
                    model,
                    host: alias_host,
                },
            ) => ModelSpec::Full {
                model: model.clone(),
                host: self_host
                    .as_ref()
                    .map(|h| h.clone())
                    .or_else(|| alias_host.clone()),
            },
        }
    }
}

impl PartialEq for ModelSpec {
    fn eq(&self, other: &Self) -> bool {
        self.model_name() == other.model_name() && self.host_name() == other.host_name()
    }
}

impl Default for ModelSpec {
    fn default() -> Self {
        Self::Plain(DEFAULT_MODEL.to_string())
    }
}

impl From<String> for ModelSpec {
    fn from(value: String) -> Self {
        Self::Plain(value)
    }
}

#[derive(Deserialize, Debug, Default, Clone)]
#[cfg_attr(test, derive(PartialEq))]
pub struct ModelOptionsInput {
    pub model: Option<ModelSpec>,
    pub lm_studio_host: Option<String>,
    pub ollama_host: Option<String>,
    pub temperature: Option<f32>,
    pub format: Option<OutputFormat>,
    pub top_p: Option<f32>,
    pub top_k: Option<u32>,
    pub frequency_penalty: Option<f32>,
    pub presence_penalty: Option<f32>,
    pub stop: Option<Vec<String>>,
    pub max_tokens: Option<u32>,
    /// Alias of short model names to full names, useful for ollama, for example
    #[serde(default)]
    pub alias: HashMap<String, ModelSpec>,

    #[serde(default)]
    pub context: ContextOptionsInput,
}

impl ModelOptionsInput {
    /// For any members that are `None` in this `ModelOptions`, use the value from `other`
    pub fn merge_defaults(&mut self, other: &ModelOptionsInput) {
        update_if_none(&mut self.model, &other.model);
        update_if_none(&mut self.lm_studio_host, &other.lm_studio_host);
        update_if_none(&mut self.ollama_host, &other.ollama_host);
        update_if_none(&mut self.temperature, &other.temperature);
        update_if_none(&mut self.format, &other.format);
        update_if_none(&mut self.top_p, &other.top_p);
        update_if_none(&mut self.top_k, &other.top_k);
        update_if_none(&mut self.frequency_penalty, &other.frequency_penalty);
        update_if_none(&mut self.presence_penalty, &other.presence_penalty);
        update_if_none(&mut self.stop, &other.stop);
        update_if_none(&mut self.max_tokens, &other.max_tokens);

        self.context.merge_defaults(&other.context);

        for (key, value) in &other.alias {
            if !self.alias.contains_key(key) {
                self.alias.insert(key.clone(), value.clone());
            }
        }
    }
}

#[derive(Error, Debug)]
pub enum ModelError {
    #[error("Error communicating with model API")]
    Raw,
    #[error("Unexpected problem decoding API response")]
    Deserialize,
    #[error("Error {0} communicating with model API: {1}")]
    Model(u16, String),
    #[error("Model does not exist: {0}")]
    ModelNotFound(String),
    #[error("Unable to format prompt")]
    FormatPrompt,
    #[error("Host does not support images")]
    HostDoesNotSupportImages,
}

pub fn map_model_response_err(err: ureq::Error) -> Report<ModelError> {
    match err {
        err @ ureq::Error::Transport(_) => Report::new(err).change_context(ModelError::Raw),
        ureq::Error::Status(code, response) => {
            let message = response.into_string().unwrap();
            Report::new(ModelError::Model(code, message))
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    mod host {
        use super::*;

        #[test]
        fn bad_default_host() {
            let options = ModelOptions {
                model: "a_model".to_string().into(),
                default_host: "nonexistent_host".to_string(),
                ..Default::default()
            };

            let err = options.api_host().unwrap_err();
            assert!(matches!(err, Error::UnknownModelHost(_)));
        }

        #[test]
        fn model_specifies_bad_host() {
            let options = ModelOptions {
                model: ModelSpec::Full {
                    model: "abc".into(),
                    host: Some("nonexistent_host".into()),
                },
                ..Default::default()
            };

            let err = options.api_host().unwrap_err();
            assert!(matches!(err, Error::UnknownModelHost(_)));
        }

        #[test]
        fn other_default_host() {
            let options = ModelOptions {
                model: ModelSpec::Plain("a_model".to_string()),
                default_host: "openrouter".to_string(),
                ..Default::default()
            };

            let host = options.api_host().unwrap();
            // This is the easiest way to tease out the actual type.
            let host_desc = format!("{host:?}");
            assert!(host_desc.contains("OpenAiHost"));
        }
    }

    mod context_length {
        use super::*;

        fn create_options(limit: Option<usize>, reserve_output: usize) -> ModelOptions {
            ModelOptions {
                model: "gpt-3.5-turbo-16k".to_string().into(),
                context: ContextOptions {
                    limit,
                    reserve_output,
                    ..Default::default()
                },
                ..Default::default()
            }
        }

        #[test]
        fn limit_smaller_than_model() {
            let options = create_options(Some(10), 5);
            assert_eq!(options.context_limit().unwrap(), Some(5));
        }

        #[test]
        fn limit_larger_than_model() {
            let options = create_options(Some(10485760), 5);
            assert_eq!(options.context_limit().unwrap(), Some(16385 - 5));
        }

        #[test]
        fn not_enough_reserved_output() {
            let options = create_options(Some(20), 20);
            let err = options.context_limit().unwrap_err();
            assert!(matches!(err.current_context(), Error::ContextLimit));
        }
    }

    mod model_spec {
        use crate::model::ModelSpec;

        #[test]
        fn merge_full_with_plain_alias() {
            let config = ModelSpec::Full {
                model: "abc".to_string(),
                host: Some("def".to_string()),
            };
            let alias = ModelSpec::Plain("ghi".to_string());

            let result = config.merge_with_alias_spec(&alias);
            assert_eq!(
                result,
                ModelSpec::Full {
                    model: "ghi".to_string(),
                    host: Some("def".to_string())
                }
            );
        }

        #[test]
        fn merge_plain_with_plain_alias() {
            let config = ModelSpec::Plain("abc".to_string());
            let alias = ModelSpec::Plain("ghi".to_string());

            let result = config.merge_with_alias_spec(&alias);
            assert_eq!(result, ModelSpec::Plain("ghi".to_string()));
        }

        #[test]
        fn merge_full_with_full_alias() {
            let config = ModelSpec::Full {
                model: "abc".to_string(),
                host: Some("def".to_string()),
            };
            let alias = ModelSpec::Full {
                model: "ghi".to_string(),
                host: Some("jkl".to_string()),
            };

            let result = config.merge_with_alias_spec(&alias);
            assert_eq!(
                result,
                ModelSpec::Full {
                    model: "ghi".to_string(),
                    host: Some("def".to_string())
                }
            );
        }

        #[test]
        fn merge_plain_with_full_alias() {
            let config = ModelSpec::Plain("abc".to_string());
            let alias = ModelSpec::Full {
                model: "ghi".to_string(),
                host: Some("jkl".to_string()),
            };

            let result = config.merge_with_alias_spec(&alias);
            assert_eq!(
                result,
                ModelSpec::Full {
                    model: "ghi".to_string(),
                    host: Some("jkl".to_string())
                }
            );
        }
    }
}
