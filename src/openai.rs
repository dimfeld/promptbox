use serde_json::json;

use crate::model::ModelOptions;

pub fn send_chat_request(
    options: &ModelOptions,
    key: &str,
    prompt: &str,
    tx: flume::Sender<String>,
) {
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
    send_request(url, key, body, tx)
}

pub fn send_instruct_request(
    options: &ModelOptions,
    key: &str,
    prompt: &str,
    tx: flume::Sender<String>,
) {
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

    send_request(url, key, body, tx)
}

fn send_request(url: &str, key: &str, body: serde_json::Value, tx: flume::Sender<String>) {
    let response = ureq::post(url)
        .set("Authorization", &format!("Bearer {}", key))
        .send_json(body);
}
