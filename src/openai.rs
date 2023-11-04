use error_stack::{Report, ResultExt};
use serde::Deserialize;
use serde_json::json;
use thiserror::Error;

use crate::model::ModelOptions;

const OPENAI_HOST: &str = "https://api.openai.com";

pub fn api_host(config: &ModelOptions) -> &str {
    if config.model == "gpt4" || config.model.starts_with("gpt-3.5-") {
        OPENAI_HOST
    } else {
        config
            .local_openai_host
            .as_deref()
            .unwrap_or("http://localhost:1234")
    }
}

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

#[derive(Error, Debug)]
pub enum OpenAiError {
    #[error("Error communicating with model API")]
    Raw,
    #[error("Unexpected problem decoding API response")]
    Deserialize,
    #[error("Error {0} communicating with model API: {1}")]
    OpenAi(u16, String),
}

fn create_base_request(config: &ModelOptions, path: &str) -> ureq::Request {
    let url = format!("{host}/{path}", host = api_host(config));

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
) -> Result<String, Report<OpenAiError>> {
    let mut body = json!({
        "model": options.model,
        "temperature": options.temperature,
        "user": "promptbox",
        "messages": [
            {
                "role": "user",
                "content": prompt,
            }
        ]
    });

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

    let mut response: ChatCompletion = create_base_request(&options, "v1/chat/completions")
        .send_json(body)
        .map_err(|e| match e {
            e @ ureq::Error::Transport(_) => Report::new(e).change_context(OpenAiError::Raw),
            ureq::Error::Status(code, response) => {
                let message = response.into_string().unwrap();
                Report::new(OpenAiError::OpenAi(code, message))
            }
        })?
        .into_json()
        .change_context(OpenAiError::Deserialize)?;

    Ok(response
        .choices
        .get_mut(0)
        .map(|m| m.message.content.take().unwrap_or_default())
        .unwrap_or_default())
}

pub fn send_completion_request(options: &ModelOptions, prompt: &str) -> Result<(), ureq::Error> {
    unimplemented!("the send_request function does not handle this response yet");
    let body = json!({
        "model": options.model,
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
