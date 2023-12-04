use error_stack::Report;

use crate::model::{ModelError, ModelOptions};

pub mod ollama;
pub mod openai;

#[derive(Debug)]
pub struct ModelComms {
    pub host: String,
    pub module: ModelCommsModule,
}

#[derive(Debug, Clone, Copy)]
pub enum ModelCommsModule {
    OpenAi,
    Ollama,
}

impl ModelCommsModule {
    pub fn send_model_request(
        &self,
        options: &ModelOptions,
        prompt: &str,
        system: Option<&str>,
        message_tx: flume::Sender<String>,
    ) -> Result<(), Report<ModelError>> {
        match self {
            ModelCommsModule::OpenAi => {
                openai::send_chat_request(options, prompt, system, message_tx)
            }
            ModelCommsModule::Ollama => ollama::send_request(options, prompt, system, message_tx),
        }
    }
}

impl ModelComms {
    pub fn new(host: impl Into<String>, module: ModelCommsModule) -> Self {
        Self {
            host: host.into(),
            module,
        }
    }

    pub fn model_context_limit(&self, model_name: &str) -> Result<usize, Report<ModelError>> {
        match self.module {
            ModelCommsModule::Ollama => ollama::model_context_limit(&self.host, model_name),
            ModelCommsModule::OpenAi => Ok(openai::model_context_limit(model_name)),
        }
    }
}
