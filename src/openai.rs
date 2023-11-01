use serde::Deserialize;
use serde_json::json;

use crate::model::ModelOptions;

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

pub fn send_chat_request(
    options: &ModelOptions,
    key: &str,
    prompt: &str,
) -> Result<String, ureq::Error> {
    let body = json!({
        "model": options.model,
        "temperature": options.temperature,
        "max_tokens": options.max_tokens,
        "top_p": options.top_p,
        "frequency_penalty": options.frequency_penalty,
        "presence_penalty": options.presence_penalty,
        "stop": options.stop,
        "user": "movableprompt",
        "messages": [
            {
                "role": "user",
                "content": prompt,
            }
        ]
    });

    let url = "https://api.openai.com/v1/chat/completions";

    let mut response: ChatCompletion = ureq::post(url)
        .set("Authorization", &format!("Bearer {}", key))
        .send_json(body)?
        .into_json()?;

    Ok(response
        .choices
        .get_mut(0)
        .map(|m| m.message.content.take().unwrap_or_default())
        .unwrap_or_default())
}

pub fn send_completion_request(
    options: &ModelOptions,
    key: &str,
    prompt: &str,
) -> Result<(), ureq::Error> {
    unimplemented!("the send_request function does not handle this response yet");
    let body = json!({
        "model": options.model,
        "temperature": options.temperature,
        "max_tokens": options.max_tokens,
        "top_p": options.top_p,
        "frequency_penalty": options.frequency_penalty,
        "presence_penalty": options.presence_penalty,
        "stop": options.stop,
        "user": "movableprompt",
        "prompt": prompt
    });

    let url = "https://api.openai.com/v1/completions";

    let response: serde_json::Value = ureq::post(url)
        .set("Authorization", &format!("Bearer {}", key))
        .send_json(body)?
        .into_json()?;
}
