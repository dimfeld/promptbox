use error_stack::Report;

use crate::model::{ModelError, ModelOptions};

pub mod ollama;
pub mod openai;

pub trait ModelHost {
    fn send_model_request(
        &self,
        options: &ModelOptions,
        prompt: &str,
        system: Option<&str>,
        message_tx: flume::Sender<String>,
    ) -> Result<(), Report<ModelError>>;

    fn model_context_limit(&self, model_name: &str) -> Result<usize, Report<ModelError>>;
}
