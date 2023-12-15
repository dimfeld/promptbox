use std::time::Duration;

use error_stack::{Report, ResultExt};
use serde::Deserialize;
use serde_json::json;

use super::{ModelHost, ModelInput};
use crate::{
    model::{map_model_response_err, ModelError, ModelOptions},
    requests::request_with_retry,
};

pub const OPENAI_HOST: &str = "https://api.openai.com/v1";

#[derive(Debug)]
pub struct OpenAiHost {
    pub api_key: Option<String>,
    pub host: Option<String>,
    /// Whether or not to check and enforce a context length limit. Usually this is true, but some
    /// hosts don't provide context length limit information or otherwise manage it themselves.
    pub do_context_limit: bool,
    pub send_user: bool,
}

impl OpenAiHost {
    pub fn new(
        host: Option<String>,
        api_key: Option<String>,
        do_context_limit: bool,
        send_user: bool,
    ) -> Self {
        Self {
            api_key,
            host,
            do_context_limit,
            send_user,
        }
    }

    fn host(&self) -> &str {
        self.host.as_deref().unwrap_or(OPENAI_HOST)
    }

    fn create_base_request(&self, path: &str) -> ureq::Request {
        let url = format!("{}/{path}", self.host());

        let request = ureq::post(&url);
        if let Some(key) = self.api_key.as_ref() {
            request.set("Authorization", &format!("Bearer {}", key))
        } else {
            request
        }
    }
}

impl ModelHost for OpenAiHost {
    fn send_model_request(
        &self,
        options: &ModelOptions,
        input: ModelInput,
        message_tx: flume::Sender<String>,
    ) -> Result<(), Report<ModelError>> {
        let user_content = if input.images.is_empty() {
            json!(input.prompt)
        } else {
            let mut messages = vec![json!({
                "type": "text",
                "text": input.prompt
            })];

            for image in &input.images {
                messages.push(json!({
                    "type": "image_url",
                    "image_url": {
                        "url": image.as_data_url()
                    }
                }));
            }

            json!(messages)
        };

        let messages = if let Some(system) = input.system {
            json!([
                {
                    "role": "system",
                    "content": system,
                },
                {
                    "role": "user",
                    "content": user_content,
                }
            ])
        } else {
            json!([
                {
                    "role": "user",
                    "content": user_content,
                }
            ])
        };

        let mut body = json!({
            "model": options.full_model_spec().model_name(),
            "temperature": options.temperature,
            "messages": messages
        });

        if self.send_user {
            body["user"] = json!("promptbox");
        }

        if let Some(val) = options.format.as_ref() {
            body["format"] = json!(val);
        }

        if let Some(val) = options.presence_penalty.as_ref() {
            body["presence_penalty"] = json!(val);
        }

        if let Some(val) = options.frequency_penalty.as_ref() {
            body["frequency_penalty"] = json!(val);
        }

        if let Some(tp) = options.top_p.as_ref() {
            body["top_p"] = json!(tp);
        }

        if !options.stop.is_empty() {
            body["stop"] = json!(options.stop);
        }

        if let Some(max_tokens) = options.max_tokens.as_ref() {
            body["max_tokens"] = json!(max_tokens);
        }

        let mut response: ChatCompletion = request_with_retry(
            self.create_base_request("chat/completions")
                .timeout(Duration::from_secs(30)),
            body,
        )
        .map_err(map_model_response_err)?
        .into_json()
        .change_context(ModelError::Deserialize)?;

        // TODO streaming
        let result = response
            .choices
            .get_mut(0)
            .map(|m| m.message.content.take().unwrap_or_default())
            .unwrap_or_default();

        message_tx.send(result).ok();
        Ok(())
    }

    fn model_context_limit(&self, model_name: &str) -> Result<Option<usize>, Report<ModelError>> {
        if self.do_context_limit {
            Ok(Some(model_context_limit(model_name)))
        } else {
            Ok(None)
        }
    }
}

#[derive(Debug, Deserialize)]
struct ChatCompletionMessage {
    role: String,
    content: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ChatCompletionChoice {
    finish_reason: Option<String>,
    index: Option<i32>,
    message: ChatCompletionMessage,
}

#[derive(Debug, Deserialize)]
struct ChatCompletion {
    id: String,
    choices: Vec<ChatCompletionChoice>,
    created: i64,
    // usage: Usage,
}

fn send_completion_request(options: &ModelOptions, prompt: &str) -> Result<(), ureq::Error> {
    unimplemented!("the send_request function does not handle this response yet");
    // let body = json!({
    //     "model": options.full_model_name(),
    //     "temperature": options.temperature,
    //     "max_tokens": options.max_tokens,
    //     "top_p": options.top_p,
    //     "frequency_penalty": options.frequency_penalty,
    //     "presence_penalty": options.presence_penalty,
    //     "stop": options.stop,
    //     "user": "promptbox",
    //     "prompt": prompt
    // });

    // let response: serde_json::Value = create_base_request(&options, "completions")
    //     .send_json(body)?
    //     .into_json()?;
}

fn model_context_limit(model_name: &str) -> usize {
    // This will all have to be updated once the preview models are productionized.
    if model_name.starts_with("gpt-4") {
        if model_name.starts_with("gpt-4-32k") {
            32768
        } else if model_name.ends_with("preview") {
            128000
        } else {
            8192
        }
    } else if model_name.contains("-16k") || model_name == "gpt-3.5-turbo-1106" {
        // gpt-3.5-turbo will also be 16385 on December 11, 2023
        16385
    } else {
        4096
    }
}

#[cfg(test)]
mod test {
    use super::model_context_limit;

    /// Check against a bunch of real models to make sure the logic is right
    #[test]
    fn model_context_values() {
        assert_eq!(model_context_limit("gpt-3.5-turbo"), 4096);
        assert_eq!(model_context_limit("gpt-3.5-turbo-16k"), 16385);
        assert_eq!(model_context_limit("gpt-3.5-turbo-1106"), 16385);
        assert_eq!(model_context_limit("gpt-3.5-turbo-instruct"), 4096);
        assert_eq!(model_context_limit("gpt-3.5-turbo-0613"), 4096);
        assert_eq!(model_context_limit("gpt-4-1106-preview"), 128000);
        assert_eq!(model_context_limit("gpt-4-vision-preview"), 128000);
        assert_eq!(model_context_limit("gpt-4"), 8192);
        assert_eq!(model_context_limit("gpt-4-0613"), 8192);
        assert_eq!(model_context_limit("gpt-4-32k"), 32768);
        assert_eq!(model_context_limit("gpt-4-32k-0613"), 32768);
    }
}
