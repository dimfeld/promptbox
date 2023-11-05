use error_stack::Report;
use serde::{Deserialize, Serialize};

use crate::model::{handle_model_response, ModelError, ModelOptions};

#[derive(Debug, Serialize)]
pub struct OllamaRequest<'a> {
    pub model: &'a str,
    pub prompt: &'a str,
    pub stream: bool,
    // TODO model parameters
}

#[derive(Deserialize)]
struct OllamaResponse {
    response: String,
    // done: bool,
    // TODO Add response stats
}

pub fn send_request(options: &ModelOptions, prompt: &str) -> Result<String, Report<ModelError>> {
    let (host, _) = options.api_host();
    let url = format!("{host}/api/generate");
    let response: OllamaResponse =
        handle_model_response(ureq::post(&url).send_json(OllamaRequest {
            model: &options.full_model_name(),
            prompt,
            stream: false,
        }))?;

    Ok(response.response)
}
