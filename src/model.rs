use serde::{Deserialize, Serialize};

use crate::{args::GlobalRunArgs, config::merge_option};

#[derive(Debug, Clone, Serialize)]
pub struct ModelOptions {
    pub model: String,
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
        merge_args_option(&mut self.model, &args.model);
        merge_args_option(&mut self.temperature, &args.temperature);
    }
}

fn merge_args_option<T: Clone>(self_value: &mut T, other_value: &Option<T>) {
    if let Some(value) = other_value.as_ref() {
        *self_value = value.clone();
    }
}

impl From<ModelOptionsInput> for ModelOptions {
    fn from(value: ModelOptionsInput) -> Self {
        Self {
            model: value.model.unwrap_or_else(|| DEFAULT_MODEL.to_string()),
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
        merge_option(&mut self.model, &other.model);
        merge_option(&mut self.temperature, &other.temperature);
        merge_option(&mut self.top_p, &other.top_p);
        merge_option(&mut self.frequency_penalty, &other.frequency_penalty);
        merge_option(&mut self.presence_penalty, &other.presence_penalty);
        merge_option(&mut self.stop, &other.stop);
        merge_option(&mut self.max_tokens, &other.max_tokens);
    }
}
