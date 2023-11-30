use std::{collections::HashMap, str::FromStr};

use error_stack::{Report, ResultExt};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    args::GlobalRunArgs,
    context::{ContextOptions, ContextOptionsInput, OverflowKeep},
    ollama,
    option::{overwrite_from_option, overwrite_option_from_option, update_if_none},
};

#[derive(Debug)]
pub struct ModelComms {
    pub host: String,
    pub module: ModelCommsModule,
}

#[derive(Debug, Clone, Copy)]
pub enum ModelCommsModule {
    OpenAi,
    Ollama,
}

impl ModelComms {
    pub fn new(host: impl Into<String>, module: ModelCommsModule) -> Self {
        Self {
            host: host.into(),
            module,
        }
    }

    pub fn model_context_limit(&self, model_name: &str) -> Result<usize, Report<ModelError>> {
        match self.module {
            ModelCommsModule::Ollama => crate::ollama::model_context_limit(&self.host, model_name),
            ModelCommsModule::OpenAi => Ok(crate::openai::model_context_limit(model_name)),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ModelOptions {
    pub model: String,
    pub lm_studio_host: Option<String>,
    pub ollama_host: Option<String>,
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
    pub alias: HashMap<String, String>,

    #[serde(default)]
    pub context: ContextOptions,
}

const DEFAULT_MODEL: &str = "gpt-3.5-turbo";
const DEFAULT_TEMPERATURE: f32 = 0.0;

impl Default for ModelOptions {
    fn default() -> Self {
        Self {
            model: DEFAULT_MODEL.to_string(),
            lm_studio_host: None,
            ollama_host: None,
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
        }
    }
}

impl ModelOptions {
    pub fn update_from_args(&mut self, args: &GlobalRunArgs) {
        overwrite_from_option(&mut self.model, &args.model);
        overwrite_option_from_option(&mut self.lm_studio_host, &args.lm_studio_host);
        overwrite_option_from_option(&mut self.ollama_host, &args.ollama_host);
        overwrite_from_option(&mut self.temperature, &args.temperature);
        overwrite_option_from_option(&mut self.format, &args.format);
        overwrite_from_option(&mut self.context.keep, &args.overflow_keep);
        overwrite_option_from_option(&mut self.context.limit, &args.context_limit);

        // Always overwrite this since there's no other way to set the key.
        self.openai_key = args.openai_key.clone();
    }

    pub fn full_model_name(&self) -> &str {
        self.alias.get(&self.model).unwrap_or(&self.model)
    }

    pub fn api_host(&self) -> ModelComms {
        let model = self.full_model_name();
        if model.starts_with("gpt-4") || model.starts_with("gpt-3.5-") {
            ModelComms::new(crate::openai::OPENAI_HOST, ModelCommsModule::OpenAi)
        } else if model == "lm-studio" {
            let host = self
                .lm_studio_host
                .as_deref()
                .unwrap_or("http://localhost:1234");
            ModelComms::new(host, ModelCommsModule::OpenAi)
        } else {
            let host = self.ollama_host.as_deref().unwrap_or(ollama::DEFAULT_HOST);
            ModelComms::new(host, ModelCommsModule::Ollama)
        }
    }

    pub fn update_from_model_input(&mut self, other: &ModelOptionsInput) {
        overwrite_from_option(&mut self.model, &other.model);
        overwrite_option_from_option(&mut self.lm_studio_host, &other.lm_studio_host);
        overwrite_option_from_option(&mut self.ollama_host, &other.ollama_host);
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

    /// Get the context size limit for a model. This may do a network request for Ollama models.
    pub fn context_limit(&self) -> Result<Option<usize>, Report<ModelError>> {
        let model = self.full_model_name();

        if model == "lm-studio" {
            // LM Studio manages this itself and doesn't expose this info via its API.
            return Ok(None);
        }

        let comms = self.api_host();
        let limit = comms.model_context_limit(model)?;

        Ok(Some(std::cmp::min(
            limit,
            self.context.limit.unwrap_or(usize::MAX),
        )))
    }
}

impl From<ModelOptionsInput> for ModelOptions {
    fn from(value: ModelOptionsInput) -> Self {
        Self {
            model: value.model.unwrap_or_else(|| DEFAULT_MODEL.to_string()),
            // For security, don't allow setting openAI key in normal config or template files.
            openai_key: None,
            lm_studio_host: value.lm_studio_host,
            ollama_host: value.ollama_host,
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
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
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

#[derive(Deserialize, Debug, Default, Clone)]
pub struct ModelOptionsInput {
    pub model: Option<String>,
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
    pub alias: HashMap<String, String>,

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

pub fn send_model_request(
    options: &ModelOptions,
    prompt: &str,
    system: &str,
    message_tx: flume::Sender<String>,
) -> Result<(), Report<ModelError>> {
    let ModelComms { module, .. } = options.api_host();
    let system = if system.is_empty() {
        None
    } else {
        Some(system)
    };

    match module {
        ModelCommsModule::OpenAi => {
            crate::openai::send_chat_request(options, prompt, system, message_tx)
        }
        ModelCommsModule::Ollama => {
            crate::ollama::send_request(options, prompt, system, message_tx)
        }
    }
}
