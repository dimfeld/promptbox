use std::collections::HashMap;

use error_stack::Report;
use serde::Deserialize;

use crate::{
    error::Error,
    model::{ModelError, ModelOptions},
    option::{overwrite_from_option, overwrite_option_from_option},
};

pub mod ollama;
pub mod openai;

pub trait ModelHost: std::fmt::Debug {
    fn send_model_request(
        &self,
        options: &ModelOptions,
        prompt: &str,
        system: Option<&str>,
        message_tx: flume::Sender<String>,
    ) -> Result<(), Report<ModelError>>;

    fn model_context_limit(&self, model_name: &str) -> Result<Option<usize>, Report<ModelError>>;
}

/// An API definition to talk to a host send prompts to it.
#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "snake_case")]
pub enum HostProtocol {
    Ollama,
    #[serde(rename = "openai")]
    OpenAi,
}

impl HostProtocol {
    fn default_context_length_option(&self) -> bool {
        match self {
            HostProtocol::Ollama => true,
            // There's no API for getting the context length here. For real OpenAI we set this to
            // true though.
            HostProtocol::OpenAi => false,
        }
    }
}

/// An LLM host
#[derive(Deserialize, Debug, Clone)]
pub struct HostDefinition {
    pub endpoint: String,
    pub protocol: HostProtocol,
    pub limit_context_length: bool,
    /// The environment variable that holds the authentication token for this host
    pub api_key: Option<String>,
}

impl HostDefinition {
    /// Create a ModelHost from this HostDefinition
    pub fn into_model_host(&self) -> Box<dyn ModelHost> {
        let key = self
            .api_key
            .as_ref()
            .and_then(|var_name| std::env::var(var_name).ok());
        let endpoint = self.endpoint.clone();
        match self.protocol {
            HostProtocol::Ollama => Box::new(ollama::OllamaHost::new(Some(endpoint), key)),
            HostProtocol::OpenAi => Box::new(openai::OpenAiHost::new(
                Some(endpoint),
                key,
                self.limit_context_length,
            )),
        }
    }

    pub fn merge_from_input(&mut self, other: &HostDefinitionInput) {
        overwrite_from_option(&mut self.endpoint, &other.endpoint);
        overwrite_from_option(&mut self.protocol, &other.protocol);
        overwrite_option_from_option(&mut self.api_key, &other.api_key);
        overwrite_from_option(&mut self.limit_context_length, &other.limit_context_length);
    }

    pub fn default_host() -> &'static str {
        "ollama"
    }

    /// A set of built-in providers
    pub fn builtin() -> HashMap<String, HostDefinition> {
        [
            (
                "lm-studio".to_string(),
                HostDefinition {
                    endpoint: "http://localhost:1234".to_string(),
                    protocol: HostProtocol::OpenAi,
                    limit_context_length: false,
                    api_key: None,
                },
            ),
            (
                "ollama".to_string(),
                HostDefinition {
                    endpoint: ollama::DEFAULT_HOST.to_string(),
                    protocol: HostProtocol::Ollama,
                    limit_context_length: true,
                    api_key: None,
                },
            ),
            (
                "openai".to_string(),
                HostDefinition {
                    endpoint: openai::OPENAI_HOST.to_string(),
                    protocol: HostProtocol::OpenAi,
                    limit_context_length: true,
                    api_key: Some("OPENAI_API_KEY".to_string()),
                },
            ),
            (
                "openrouter".to_string(),
                HostDefinition {
                    endpoint: "https://api.openrouter.ai/api".to_string(),
                    protocol: HostProtocol::OpenAi,
                    limit_context_length: false,
                    api_key: Some("OPENROUTER_API_KEY".to_string()),
                },
            ),
        ]
        .into_iter()
        .collect()
    }
}

impl TryFrom<HostDefinitionInput> for HostDefinition {
    type Error = Error;

    /// Create a HostDefinition from a HostDefinitionInput. If there is an existing HostDefinition
    /// with the same name, use [merge_from_input] instead.
    fn try_from(value: HostDefinitionInput) -> Result<Self, Self::Error> {
        let endpoint = value.endpoint.ok_or(Error::MissingField("endpoint"))?;
        let protocol = value.protocol.ok_or(Error::MissingField("protocol"))?;
        Ok(Self {
            endpoint,
            limit_context_length: value
                .limit_context_length
                .unwrap_or_else(|| protocol.default_context_length_option()),
            protocol,
            api_key: value.api_key,
        })
    }
}

#[derive(Deserialize, Default, Debug, Clone)]
pub struct HostDefinitionInput {
    pub endpoint: Option<String>,
    pub api_key: Option<String>,
    pub protocol: Option<HostProtocol>,
    pub limit_context_length: Option<bool>,
}

impl HostDefinitionInput {
    pub fn merge_from_input(&mut self, other: &HostDefinitionInput) {
        overwrite_option_from_option(&mut self.endpoint, &other.endpoint);
        overwrite_option_from_option(&mut self.protocol, &other.protocol);
        overwrite_option_from_option(&mut self.api_key, &other.api_key);
        overwrite_option_from_option(&mut self.limit_context_length, &other.limit_context_length)
    }
}

#[cfg(test)]
mod test {
    use crate::hosts::HostDefinition;

    #[test]
    fn default_host_is_valid() {
        let builtin = super::HostDefinition::builtin();
        assert!(builtin.contains_key(HostDefinition::default_host()));
    }
}
