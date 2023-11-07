use std::io::BufRead;

use error_stack::{Report, ResultExt};
use serde::{Deserialize, Serialize};
use ureq::Response;

use crate::model::{map_model_response_err, ModelError, ModelOptions};

#[derive(Debug, Serialize)]
pub struct OllamaRequest<'a> {
    pub model: &'a str,
    pub prompt: &'a str,
    pub stream: bool,
    pub options: OllamaModelOptions,
}

#[derive(Debug, Serialize)]
pub struct OllamaModelOptions {
    temperature: f32,
    top_p: Option<f32>,
    top_k: Option<u32>,
    repeat_penalty: Option<f32>,
    num_predict: Option<u32>,
    stop: Vec<String>,
}

#[derive(Deserialize)]
struct OllamaResponse {
    response: String,
    done: bool,
    // TODO Add response stats
}

pub fn send_request(
    options: &ModelOptions,
    prompt: &str,
    message_tx: flume::Sender<String>,
) -> Result<(), Report<ModelError>> {
    let (host, _) = options.api_host();
    let url = format!("{host}/api/generate");
    let response: Response = ureq::post(&url)
        .send_json(OllamaRequest {
            model: &options.full_model_name(),
            prompt,
            options: OllamaModelOptions {
                temperature: options.temperature,
                top_p: options.top_p,
                top_k: options.top_k,
                repeat_penalty: options.frequency_penalty,
                stop: options.stop.clone(),
                num_predict: options.max_tokens,
            },
            stream: true,
        })
        .map_err(map_model_response_err)?;

    let reader = std::io::BufReader::new(response.into_reader());
    for line in reader.lines() {
        let line = line.change_context(ModelError::Raw)?;
        let chunk = serde_json::from_str::<OllamaResponse>(&line)
            .change_context(ModelError::Deserialize)?;
        message_tx.send(chunk.response).ok();
    }

    Ok(())
}
