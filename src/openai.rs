use std::time::Duration;

use error_stack::{Report, ResultExt};
use serde::Deserialize;
use serde_json::json;

use crate::{
    model::{map_model_response_err, ModelComms, ModelError, ModelOptions},
    requests::request_with_retry,
};

pub const OPENAI_HOST: &str = "https://api.openai.com";

#[derive(Debug, Deserialize)]
struct ChatCompletionMessage {
    role: String,
    content: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ChatCompletionChoice {
    finish_reason: String,
    index: i32,
    message: ChatCompletionMessage,
}

#[derive(Debug, Deserialize)]
struct ChatCompletion {
    id: String,
    choices: Vec<ChatCompletionChoice>,
    created: i64,
    // usage: Usage,
}

fn create_base_request(config: &ModelOptions, path: &str) -> ureq::Request {
    let ModelComms { host, .. } = config.api_host();
    let url = format!("{host}/{path}");

    let request = ureq::post(&url);
    if let Some(key) = config.openai_key.as_ref() {
        request.set("Authorization", &format!("Bearer {}", key))
    } else {
        request
    }
}

pub fn send_chat_request(
    options: &ModelOptions,
    prompt: &str,
    system: Option<&str>,
    message_tx: flume::Sender<String>,
) -> Result<(), Report<ModelError>> {
    let messages = if let Some(system) = system {
        json!([
            {
                "role": "system",
                "content": system,
            },
            {
                "role": "user",
                "content": prompt,
            }
        ])
    } else {
        json!([
            {
                "role": "user",
                "content": prompt,
            }
        ])
    };

    let mut body = json!({
        "model": options.full_model_name(),
        "temperature": options.temperature,
        "user": "promptbox",
        "messages": messages
    });

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
        create_base_request(&options, "v1/chat/completions").timeout(Duration::from_secs(30)),
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

pub fn send_completion_request(options: &ModelOptions, prompt: &str) -> Result<(), ureq::Error> {
    unimplemented!("the send_request function does not handle this response yet");
    let body = json!({
        "model": options.full_model_name(),
        "temperature": options.temperature,
        "max_tokens": options.max_tokens,
        "top_p": options.top_p,
        "frequency_penalty": options.frequency_penalty,
        "presence_penalty": options.presence_penalty,
        "stop": options.stop,
        "user": "promptbox",
        "prompt": prompt
    });

    let response: serde_json::Value = create_base_request(&options, "v1/completions")
        .send_json(body)?
        .into_json()?;
}

pub fn model_context_limit(model_name: &str) -> usize {
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
