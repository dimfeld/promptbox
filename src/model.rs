use serde::{Deserialize, Serialize};

use crate::{
    args::GlobalRunArgs,
    option::{overwrite_from_option, overwrite_option_from_option, update_if_none},
};

#[derive(Debug, Clone, Serialize)]
pub struct ModelOptions {
    pub model: String,
    pub local_openai_host: Option<String>,
    pub openai_key: Option<String>,
    pub temperature: f32,
    pub top_p: Option<f32>,
    pub frequency_penalty: Option<f32>,
    pub presence_penalty: Option<f32>,
    pub stop: Vec<String>,
    pub max_tokens: Option<u32>,
}

const DEFAULT_MODEL: &str = "gpt-3.5-turbo";
const DEFAULT_TEMPERATURE: f32 = 0.0;

impl Default for ModelOptions {
    fn default() -> Self {
        Self {
            model: DEFAULT_MODEL.to_string(),
            local_openai_host: None,
            openai_key: None,
            temperature: DEFAULT_TEMPERATURE,
            top_p: None,
            frequency_penalty: None,
            presence_penalty: None,
            stop: Vec::new(),
            max_tokens: None,
        }
    }
}

impl ModelOptions {
    pub fn update_from_args(&mut self, args: &GlobalRunArgs) {
        overwrite_from_option(&mut self.model, &args.model);
        overwrite_option_from_option(&mut self.local_openai_host, &args.local_openai_host);
        overwrite_from_option(&mut self.temperature, &args.temperature);

        // Always overwrite this since there's no other way to set the key.
        self.openai_key = args.openai_key.clone();
    }
}

impl From<ModelOptionsInput> for ModelOptions {
    fn from(value: ModelOptionsInput) -> Self {
        Self {
            model: value.model.unwrap_or_else(|| DEFAULT_MODEL.to_string()),
            // For security, don't allow setting openAI key in normal config or template files.
            openai_key: None,
            local_openai_host: value.local_openai_host,
            temperature: value.temperature.unwrap_or(DEFAULT_TEMPERATURE),
            top_p: value.top_p,
            frequency_penalty: value.frequency_penalty,
            presence_penalty: value.presence_penalty,
            stop: value.stop.unwrap_or_default(),
            max_tokens: value.max_tokens,
        }
    }
}

#[derive(Deserialize, Debug, Default, Clone)]
pub struct ModelOptionsInput {
    pub model: Option<String>,
    pub local_openai_host: Option<String>,
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub frequency_penalty: Option<f32>,
    pub presence_penalty: Option<f32>,
    pub stop: Option<Vec<String>>,
    pub max_tokens: Option<u32>,
}

impl ModelOptionsInput {
    /// For any members that are `None` in this `ModelOptions`, use the value from `other`
    pub fn merge_defaults(&mut self, other: &ModelOptionsInput) {
        update_if_none(&mut self.model, &other.model);
        update_if_none(&mut self.local_openai_host, &other.local_openai_host);
        update_if_none(&mut self.temperature, &other.temperature);
        update_if_none(&mut self.top_p, &other.top_p);
        update_if_none(&mut self.frequency_penalty, &other.frequency_penalty);
        update_if_none(&mut self.presence_penalty, &other.presence_penalty);
        update_if_none(&mut self.stop, &other.stop);
        update_if_none(&mut self.max_tokens, &other.max_tokens);
    }
}
