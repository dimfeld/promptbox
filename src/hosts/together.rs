use std::{borrow::Cow, cell::OnceCell, io::BufRead, time::Duration};

use error_stack::{Report, ResultExt};
use serde::{Deserialize, Serialize};
use serde_json::json;
use ureq::Response;

use super::ModelHost;
use crate::{
    cache::Cache,
    model::{map_model_response_err, ModelError, ModelOptions, OutputFormat},
    requests::{add_bearer_token, request_with_retry},
};

pub const DEFAULT_HOST: &str = "https://api.together.xyz";

#[derive(Debug)]
pub struct TogetherHost {
    pub host: String,
    pub api_key: Option<String>,

    cache: Option<Cache>,

    model_info: OnceCell<Vec<ModelInfo>>,
}

impl TogetherHost {
    pub fn new(host: String, api_key: Option<String>) -> Self {
        Self {
            host,
            api_key,
            cache: Cache::new().ok(),
            model_info: OnceCell::new(),
        }
    }

    fn host(&self) -> &str {
        &self.host
    }

    fn fetch_all_model_info(&self) -> Result<Vec<ModelInfo>, Report<ModelError>> {
        let url = format!("{}/models/info", self.host());
        add_bearer_token(ureq::get(&url), &self.api_key)
            .call()
            .map_err(map_model_response_err)
            .attach_printable(url)?
            .into_json::<Vec<ModelInfo>>()
            .change_context(ModelError::Deserialize)
    }

    fn get_all_model_info(&self) -> Result<&[ModelInfo], Report<ModelError>> {
        if let Some(model_info) = self.model_info.get() {
            return Ok(model_info);
        }

        if let Some(cache) = self.cache.as_ref() {
            let model_info: Option<Vec<ModelInfo>> = cache
                .read_cache(
                    "together_model_info.json",
                    Duration::from_secs(60 * 60 * 24),
                )
                .ok()
                .flatten();

            if let Some(model_info) = model_info {
                self.model_info.set(model_info).ok();
                return Ok(&self.model_info.get().unwrap());
            }
        }

        let model_info = self.fetch_all_model_info()?;
        if let Some(cache) = self.cache.as_ref() {
            cache
                .write_cache("together_model_info.json", &model_info)
                .ok();
        }

        self.model_info.set(model_info).ok();
        Ok(&self.model_info.get().unwrap())
    }

    fn get_model_info(&self, model: &str) -> Result<&ModelInfo, Report<ModelError>> {
        let info = self.get_all_model_info()?;
        let model_info = info
            .iter()
            .find(|i| i.name == model)
            .ok_or_else(|| ModelError::ModelNotFound(model.to_string()))?;
        Ok(model_info)
    }

    fn fuse_system_prompt<'slf, 'a>(
        &'slf self,
        prompt: &'a str,
        system: Option<&'a str>,
    ) -> Cow<'a, str> {
        if let Some(system) = system {
            Cow::Owned(format!("{}\n\n{}", system, prompt))
        } else {
            Cow::Borrowed(prompt)
        }
    }

    fn format_prompt<'slf, 'a>(
        &'slf self,
        config: &'slf ModelConfig,
        prompt: &'a str,
        system: Option<&'a str>,
    ) -> Cow<'a, str> {
        // Look at chat_template
        if let Some(template) = config.chat_template.as_ref() {
            // TODO This is jinja format, check out https://lib.rs/crates/minijinja
            todo!("Format from jinja format");
        }

        let chat_template_name = config.chat_template_name.as_deref().unwrap_or_default();
        if !chat_template_name.is_empty() && chat_template_name != "default" {
            todo!("Look at chat template name");
        }

        // If we get down here then there is no special handling of system prompt, so paste them
        // together.
        let prompt = self.fuse_system_prompt(prompt, system);

        if let Some(template) = config.prompt_format.as_ref() {
            Cow::from(template.replace("{prompt}", &prompt))
        } else {
            prompt
        }
    }
}

impl ModelHost for TogetherHost {
    fn send_model_request(
        &self,
        options: &ModelOptions,
        prompt: &str,
        system: Option<&str>,
        message_tx: flume::Sender<String>,
    ) -> Result<(), Report<ModelError>> {
        let full_spec = options.full_model_spec();
        let model_name = full_spec.model_name();
        let model_info = self.get_model_info(model_name)?;

        let prompt = self.format_prompt(&model_info.config, prompt, system);

        let mut stop = options.stop.clone();
        if let Some(model_stop) = model_info.config.stop.as_ref() {
            stop.extend(model_stop.iter().cloned());
        }

        let body = TogetherRequest {
            model: model_name,
            prompt: &prompt,
            response_format: Some(TogetherRequestFormat {
                typ: match options.format {
                    Some(OutputFormat::JSON) => "json_object",
                    _ => "text",
                },
            }),
            temperature: options.temperature,
            top_p: options.top_p,
            top_k: options.top_k,
            repetition_penalty: options.frequency_penalty,
            stop,
            max_tokens: options.max_tokens.unwrap_or(2048),
            stream: false,
        };

        let url = format!("{}/inference", self.host());
        let request = add_bearer_token(ureq::post(&url), &self.api_key);
        let mut response = request_with_retry(request, body)
            .map_err(map_model_response_err)
            .attach_printable_lazy(|| url.clone())?
            .into_json::<TogetherResponse>()
            .change_context(ModelError::Deserialize)
            .attach_printable_lazy(|| url.clone())?;

        let message = response
            .output
            .choices
            .pop()
            .map(|c| c.text)
            .unwrap_or_default();
        if !message.is_empty() {
            message_tx.send(message).ok();
        }

        Ok(())
    }

    fn model_context_limit(&self, model: &str) -> Result<Option<usize>, Report<ModelError>> {
        let model_info = self.get_model_info(model)?;
        let context_size = model_info.context_length.unwrap_or(2048);
        Ok(Some(context_size as usize))
    }
}
#[derive(Debug, Serialize)]
struct TogetherRequest<'a> {
    pub model: &'a str,
    pub prompt: &'a str,
    pub stream: bool,
    pub response_format: Option<TogetherRequestFormat>,
    pub temperature: f32,
    pub top_p: Option<f32>,
    pub top_k: Option<u32>,
    pub repetition_penalty: Option<f32>,
    pub max_tokens: u32,
    pub stop: Vec<String>,
}

#[derive(Debug, Serialize)]
struct TogetherRequestFormat {
    #[serde(rename = "type")]
    typ: &'static str,
}

#[derive(Deserialize)]
struct TogetherResponse {
    output: TogetherOutput,
    // TODO Add response stats
}

#[derive(Deserialize)]
struct TogetherOutput {
    choices: Vec<TogetherChoice>,
}

#[derive(Deserialize)]
struct TogetherChoice {
    // finish_reason: String,
    // index: i32,
    text: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct ModelInfo {
    context_length: Option<u32>,
    name: String,
    config: ModelConfig,
}

#[derive(Serialize, Deserialize, Debug)]
struct ModelConfig {
    chat_template_name: Option<String>,
    chat_template: Option<String>,
    prompt_format: Option<String>,
    stop: Option<Vec<String>>,
}

#[cfg(all(test, feature = "test-together"))]
mod test {
    use super::model_context_limit;
    use crate::hosts::ModelHost;

    #[test]
    /// Get the context size for a model that specifies it in the modelfile.
    fn model_context_with_info() {
        let host = super::TogetherHost::new(None);
        let limit = host
            .model_context_limit("yarn-mistral:7b-128k-q5_K_M")
            .expect("Fetching context");
        assert_eq!(limit, 131072);
    }

    #[test]
    /// Get the context size for a model that doesn't specify it in the modelfile.
    fn model_context_without_info() {
        let host = super::TogetherHost::new(None);
        let limit = host
            .model_context_limit("mistral:7b-instruct-q5_K_M")
            .expect("Fetching context");
        assert_eq!(limit, 2048);
    }
}
