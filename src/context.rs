use clap::ValueEnum;
use serde::{Deserialize, Serialize};
use tokenizers::{models::bpe::BPE, Encoding, Tokenizer};

use crate::{option::update_if_none, Error};

pub fn encode(input: &str) -> Result<Encoding, Error> {
    // This isn't accurate for everything but most models are using a similar config.
    let tokenizer = Tokenizer::new(BPE::default());
    tokenizer
        .encode(input, false)
        .map_err(|e| Error::Tokenizer(e.to_string()))
}

#[derive(Serialize, Deserialize, Default, Debug, Clone, Copy, ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum OverflowKeep {
    /// Keep the start of the content
    #[default]
    Start,
    /// Keep the end of the content
    End,
}

#[derive(Serialize, Deserialize, Default, Debug, Clone)]
pub struct ContextOptions {
    /// Set a lower context size limit for a model.
    pub limit: Option<usize>,
    /// Which side of the context to keep when trimming.
    pub keep: OverflowKeep,
    /// Which arguments to drop content from when the context is too large.
    /// If empty, content will be removed from the entire rendered context.
    pub trim_args: Vec<String>,
}

impl From<ContextOptionsInput> for ContextOptions {
    fn from(value: ContextOptionsInput) -> Self {
        Self {
            limit: value.limit,
            keep: value.keep.unwrap_or_default(),
            trim_args: value.trim_args,
        }
    }
}

impl ContextOptions {
    pub fn truncate_at<'a>(&self, input: &'a str, encoding: &Encoding) -> &'a str {
        let Some(limit) = self.limit else {
            return input;
        };

        if encoding.len() < limit {
            return input;
        }

        match self.keep {
            OverflowKeep::Start => {
                let end = encoding.get_offsets()[limit - 1];
                &input[0..end.1]
            }
            OverflowKeep::End => {
                let start_index = encoding.len() - limit;
                let start = encoding.get_offsets()[start_index];
                &input[start.0..]
            }
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct ContextOptionsInput {
    /// Set a lower context size limit for a model.
    limit: Option<usize>,
    /// Which side of the context to keep when we have to drop some content
    keep: Option<OverflowKeep>,
    /// Which arguments to drop content from when the context is too large.
    /// If empty, content will be removed from the entire rendered context.
    trim_args: Vec<String>,
}

impl ContextOptionsInput {
    pub fn merge_defaults(&mut self, other: &ContextOptionsInput) {
        update_if_none(&mut self.limit, &other.limit);
        update_if_none(&mut self.keep, &other.keep);

        if !other.trim_args.is_empty() {
            self.trim_args = other.trim_args.clone();
        }
    }
}
