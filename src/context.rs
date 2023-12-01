use std::path::Path;

use clap::ValueEnum;
use error_stack::{Report, ResultExt};
use liquid::ValueView;
use serde::{Deserialize, Serialize};
use tokenizers::{EncodeInput, Encoding};

use crate::{model::ModelOptions, option::update_if_none, Error};

struct Tokenizer(tokenizers::Tokenizer);

impl Tokenizer {
    fn new() -> Result<Self, Error> {
        // This isn't accurate for everything but most models are using a similar config.
        // Eventually it would be better to get the proper tokenizer for each model.
        let tokenizer = tokenizers::Tokenizer::from_pretrained("TheBloke/Llama-2-70B-fp16", None)
            .map_err(|e| Error::Tokenizer(e.to_string()))?;
        Ok(Self(tokenizer))
    }

    fn encode(&self, input: &str) -> Result<Encoding, Error> {
        self.0
            .encode(input, false)
            .map_err(|e| Error::Tokenizer(e.to_string()))
    }

    fn encode_batch<'s>(
        &self,
        input: Vec<impl Into<EncodeInput<'s>> + Send>,
    ) -> Result<Vec<Encoding>, Error> {
        self.0
            .encode_batch(input, false)
            .map_err(|e| Error::Tokenizer(e.to_string()))
    }
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

#[derive(Serialize, Deserialize, Default, Debug, Clone, Copy, ValueEnum)]
#[serde(rename_all = "snake_case")]
/// Control how array arguments are trimmed when reducing context overflow.
pub enum ArrayTrimPriority {
    /// Preserve the start of the array, when possible
    #[default]
    First,
    /// Preserve the end of the array, when possible
    Last,
    /// Trim an equal amount off of each argument.
    Equal,
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
    /// When trimming array arguments, whether to trim from the first arguments,
    /// the last arguments, or try to trim equally.
    pub array_priority: ArrayTrimPriority,
}

impl From<ContextOptionsInput> for ContextOptions {
    fn from(value: ContextOptionsInput) -> Self {
        Self {
            limit: value.limit,
            keep: value.keep.unwrap_or_default(),
            trim_args: value.trim_args,
            array_priority: value.array_priority.unwrap_or_default(),
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
    /// When trimming array arguments, whether to trim from the first arguments,
    /// the last arguments, or try to trim equally.
    array_priority: Option<ArrayTrimPriority>,
}

impl ContextOptionsInput {
    pub fn merge_defaults(&mut self, other: &ContextOptionsInput) {
        update_if_none(&mut self.limit, &other.limit);
        update_if_none(&mut self.keep, &other.keep);
        update_if_none(&mut self.array_priority, &other.array_priority);

        if !other.trim_args.is_empty() {
            self.trim_args = other.trim_args.clone();
        }
    }
}

fn truncate_at<'a>(
    limit: usize,
    keep: OverflowKeep,
    input: &'a str,
    encoding: &Encoding,
) -> &'a str {
    if encoding.len() < limit {
        return input;
    }

    match keep {
        OverflowKeep::Start => {
            let end = encoding.get_offsets()[limit - 1];
            &input[0..end.1].trim_end()
        }
        OverflowKeep::End => {
            let start_index = encoding.len() - limit;
            let start = encoding.get_offsets()[start_index];
            &input[start.0..].trim_start()
        }
    }
}

pub fn enforce_context_limit(
    model_options: &ModelOptions,
    parser: &liquid::Parser,
    template_path: &Path,
    template: &str,
    mut template_args: liquid::Object,
    rendered: String,
) -> Result<String, Report<Error>> {
    let context_limit = model_options
        .context_limit()
        .change_context(Error::PreparePrompt)?;

    let Some(context_limit) = context_limit else {
        return Ok(rendered);
    };

    let tokenizer = Tokenizer::new().change_context(Error::PreparePrompt)?;
    let encoded = tokenizer
        .encode(&rendered)
        .change_context(Error::PreparePrompt)?;

    if encoded.len() <= context_limit {
        return Ok(rendered);
    }

    if model_options.context.trim_args.is_empty() {
        // trim from the entire context
        let prompt = truncate_at(
            context_limit,
            model_options.context.keep,
            &rendered,
            &encoded,
        )
        .to_string();
        Ok(prompt)
    } else {
        // trim from specific arguments and rerender
        trim_context_from_args(
            &tokenizer,
            context_limit,
            encoded.len(),
            &model_options.context,
            &mut template_args,
        )?;

        let prompt =
            crate::template::render_template(parser, template_path, template, &template_args)?;

        Ok(prompt)
    }
}

fn trim_context_from_args(
    tokenizer: &Tokenizer,
    context_limit: usize,
    current_tokens: usize,
    context_options: &ContextOptions,
    template_args: &mut liquid::Object,
) -> Result<(), Report<Error>> {
    let mut to_trim = (current_tokens - context_limit) as isize;

    for arg in &context_options.trim_args {
        if to_trim <= 0 {
            break;
        }

        if let Some(value) = template_args.get_mut(arg.as_str()) {
            let trimmed_amount =
                trim_arg(tokenizer, to_trim as usize, context_options, None, value)?;
            to_trim -= trimmed_amount as isize;
        }
    }

    Ok(())
}

fn trim_arg(
    tokenizer: &Tokenizer,
    to_trim: usize,
    context_options: &ContextOptions,
    encoded_value: Option<Encoding>,
    value: &mut liquid::model::Value,
) -> Result<usize, Report<Error>> {
    if to_trim == 0 {
        return Ok(0);
    }

    match value {
        liquid::model::Value::Array(array) => {
            let mut remaining_to_trim = to_trim as isize;
            let mut total_trimmed = 0;
            match context_options.array_priority {
                ArrayTrimPriority::First => {
                    for value in array.iter_mut().rev() {
                        if remaining_to_trim <= 0 {
                            break;
                        }

                        let trimmed = trim_arg(
                            tokenizer,
                            remaining_to_trim as usize,
                            context_options,
                            None,
                            value,
                        )?;
                        total_trimmed += trimmed;
                        remaining_to_trim -= trimmed as isize;
                    }
                }
                ArrayTrimPriority::Last => {
                    for value in array.iter_mut() {
                        if remaining_to_trim <= 0 {
                            break;
                        }

                        let trimmed = trim_arg(tokenizer, to_trim, context_options, None, value)?;
                        total_trimmed += trimmed;
                        remaining_to_trim -= trimmed as isize;
                    }
                }
                ArrayTrimPriority::Equal => {
                    let encoded = array
                        .iter()
                        .map(|v| {
                            let s = v.to_kstr();
                            tokenizer.encode(s.as_str())
                        })
                        .collect::<Result<Vec<_>, _>>()?;

                    // Trim an equal percentage from each value.
                    let total_tokens = encoded.iter().map(|e| e.len()).sum::<usize>();
                    let percent_to_trim = to_trim as f32 / total_tokens as f32;

                    for (value, encoded) in array.iter_mut().zip(encoded.into_iter()) {
                        let this_to_trim =
                            (encoded.len() as f32 * percent_to_trim).round() as usize;
                        if this_to_trim > 0 {
                            trim_arg(
                                tokenizer,
                                this_to_trim,
                                context_options,
                                Some(encoded),
                                value,
                            )?;
                        }
                    }
                }
            }

            array.retain(|f| !f.to_kstr().as_str().is_empty());
            Ok(total_trimmed)
        }
        liquid::model::Value::Scalar(s) => {
            let value = s.to_kstr();
            let encoded = encoded_value
                .map(Ok)
                .unwrap_or_else(|| tokenizer.encode(value.as_str()))?;

            if encoded.len() > to_trim {
                let trimmed = truncate_at(
                    encoded.len() - to_trim,
                    context_options.keep,
                    value.as_str(),
                    &encoded,
                );
                let new_str = trimmed.to_string();
                *s = new_str.into();
                Ok(to_trim)
            } else {
                // Removing the entire value. This will get filtered out properly later.
                *s = "".into();
                Ok(encoded.len())
            }
        }
        _ => Ok(0),
    }
}

#[cfg(test)]
mod test {
    use super::*;

    const SAMPLE_TEXT_1: &str = "This is a test texting and it is full of sample text";
    const SAMPLE_TEXT_2: &str = "Another test text too!";
    const SAMPLE_TEXT_3: &str = "Testing testers test";
    // Calculated from the three texts together
    const TOTAL_TOKENS: usize = 23;

    mod enforce_context_limit {
        use std::path::PathBuf;

        use liquid::{
            object,
            partials::{InMemorySource, LazyCompiler},
        };

        use super::*;

        fn init_test(limit: usize) -> (ModelOptions, liquid::Object, liquid::Parser, String) {
            let model_options = ModelOptions {
                model: "gpt-3.5-turbo".to_string(),
                context: ContextOptions {
                    limit: Some(limit),
                    ..Default::default()
                },
                ..Default::default()
            };

            let template_context = object!({
                "title": "My blog",
                "extra": "Some blog post with a lot of content to summarize"
            });

            let parser = liquid::ParserBuilder::<LazyCompiler<InMemorySource>>::default()
                .stdlib()
                .build()
                .expect("failed to build parser");

            let initial_render = parser
                .parse(TEST_TEMPLATE)
                .unwrap()
                .render(&template_context)
                .unwrap();

            (model_options, template_context, parser, initial_render)
        }

        const TEST_TEMPLATE: &str = r##"
            This is a document to summarize titled {{title}}.

            {{extra}}

            The summary is:
            "##;

        #[test]
        fn below_limit() {
            let (options, context, parser, initial_render) = init_test(2048);

            let output = enforce_context_limit(
                &options,
                &parser,
                &PathBuf::from("test"),
                TEST_TEMPLATE,
                context,
                initial_render.clone(),
            )
            .unwrap();

            assert_eq!(output, initial_render);
        }

        #[test]
        fn above_limit_with_trim_args() {
            let (mut options, context, parser, initial_render) = init_test(33);

            options.context.trim_args = vec!["extra".to_string()];

            let output = enforce_context_limit(
                &options,
                &parser,
                &PathBuf::from("test"),
                TEST_TEMPLATE,
                context,
                initial_render.clone(),
            )
            .unwrap();

            let expected_context = object!({
                "title": "My blog",
                "extra": "Some blog post with a lot of"
            });

            let expected_render = parser
                .parse(TEST_TEMPLATE)
                .unwrap()
                .render(&expected_context)
                .unwrap();

            assert_eq!(output, expected_render);
        }

        #[test]
        fn above_limit_without_trim_args() {
            let (mut options, context, parser, initial_render) = init_test(30);
            options.context.keep = OverflowKeep::End;

            let output = enforce_context_limit(
                &options,
                &parser,
                &PathBuf::from("test"),
                TEST_TEMPLATE,
                context,
                initial_render.clone(),
            )
            .unwrap();

            assert_eq!(output, &initial_render[32..]);
        }
    }

    mod truncate_at {
        use super::*;

        #[test]
        fn truncate_start() {
            let tokenizer = Tokenizer::new().unwrap();
            let result = truncate_at(
                6,
                OverflowKeep::Start,
                SAMPLE_TEXT_1,
                &tokenizer.encode(SAMPLE_TEXT_1).unwrap(),
            );
            assert_eq!(result, "This is a test texting");
        }

        #[test]
        fn truncate_end() {
            let tokenizer = Tokenizer::new().unwrap();
            let result = truncate_at(
                6,
                OverflowKeep::End,
                SAMPLE_TEXT_1,
                &tokenizer.encode(SAMPLE_TEXT_1).unwrap(),
            );
            assert_eq!(result, "it is full of sample text");
        }
    }

    mod trim_context_from_args {
        use liquid::object;

        use super::*;

        fn sum_tokens(tokenizer: &Tokenizer, values: &liquid::model::Value) -> usize {
            let inputs = values
                .as_array()
                .unwrap()
                .values()
                .map(|v| v.to_kstr().as_str().to_string())
                .collect::<Vec<_>>();

            tokenizer
                .encode_batch(inputs)
                .unwrap()
                .into_iter()
                .map(|e| e.len())
                .sum()
        }

        /// Trim a scalar value, and that only the value in trim_args gets trimmed.
        #[test]
        fn trim_scalar_value() {
            let tokenizer = Tokenizer::new().unwrap();
            let mut args = object!({
                "another_value": 5,
                "a_title": "The Wizard of Oz",
                "test": SAMPLE_TEXT_1,
                "zoos": "animals"
            });

            trim_context_from_args(
                &tokenizer,
                13,
                18,
                &ContextOptions {
                    limit: None,
                    keep: OverflowKeep::Start,
                    trim_args: vec!["test".to_string()],
                    array_priority: ArrayTrimPriority::First,
                },
                &mut args,
            )
            .unwrap();

            assert_eq!(
                args,
                object!({
                    "another_value": 5,
                    "a_title": "The Wizard of Oz",
                    "test": "This is a test texting and it",
                    "zoos": "animals"
                })
            );
        }

        /// Trim array values when multiple values get trimmed.
        #[test]
        fn trim_array_value_first_multiple_values() {
            let tokenizer = Tokenizer::new().unwrap();
            let mut args = object!({
                "test": vec![
                    SAMPLE_TEXT_1,
                    SAMPLE_TEXT_2,
                    SAMPLE_TEXT_3,
                ],
            });

            let start_tokens = sum_tokens(&tokenizer, &args["test"]);
            println!("start tokens: {}", start_tokens);

            trim_context_from_args(
                &tokenizer,
                TOTAL_TOKENS - 7,
                TOTAL_TOKENS,
                &ContextOptions {
                    limit: None,
                    keep: OverflowKeep::Start,
                    trim_args: vec!["test".to_string()],
                    array_priority: ArrayTrimPriority::First,
                },
                &mut args,
            )
            .unwrap();

            assert_eq!(
                args,
                object!({
                    "test": vec![
                        SAMPLE_TEXT_1,
                        "Another test text",
                    ],
                })
            );

            let total_tokens = sum_tokens(&tokenizer, &args["test"]);
            assert!(total_tokens == TOTAL_TOKENS - 7);
        }

        /// Trim array values when a single value gets trimmed completely out.
        #[test]
        fn trim_array_value_first_single_value_exact() {
            let tokenizer = Tokenizer::new().unwrap();
            let mut args = object!({
                "test": vec![
                    SAMPLE_TEXT_1,
                    SAMPLE_TEXT_2,
                    SAMPLE_TEXT_3,
                ],
            });

            trim_context_from_args(
                &tokenizer,
                TOTAL_TOKENS - 5,
                TOTAL_TOKENS,
                &ContextOptions {
                    limit: None,
                    keep: OverflowKeep::Start,
                    trim_args: vec!["test".to_string()],
                    array_priority: ArrayTrimPriority::First,
                },
                &mut args,
            )
            .unwrap();

            assert_eq!(
                args,
                object!({
                    "test": vec![
                        SAMPLE_TEXT_1,
                        SAMPLE_TEXT_2,
                    ],
                })
            );

            let total_tokens = sum_tokens(&tokenizer, &args["test"]);
            assert!(total_tokens == TOTAL_TOKENS - 5);
        }

        /// Test trimming array values when a single value gets trimmed partially.
        #[test]
        fn trim_array_value_first_single_value_partial() {
            let tokenizer = Tokenizer::new().unwrap();
            let mut args = object!({
                "test": vec![
                    SAMPLE_TEXT_1,
                    SAMPLE_TEXT_2,
                    SAMPLE_TEXT_3,
                ],
            });

            trim_context_from_args(
                &tokenizer,
                TOTAL_TOKENS - 2,
                TOTAL_TOKENS,
                &ContextOptions {
                    limit: None,
                    keep: OverflowKeep::Start,
                    trim_args: vec!["test".to_string()],
                    array_priority: ArrayTrimPriority::First,
                },
                &mut args,
            )
            .unwrap();

            assert_eq!(
                args,
                object!({
                    "test": vec![
                        SAMPLE_TEXT_1,
                        SAMPLE_TEXT_2,
                        "Testing test"
                    ],
                })
            );

            let total_tokens = sum_tokens(&tokenizer, &args["test"]);
            assert!(total_tokens == TOTAL_TOKENS - 2);
        }

        /// Trim array values when keeping the last values
        #[test]
        fn trim_array_value_last() {
            let tokenizer = Tokenizer::new().unwrap();
            let mut args = object!({
                "test": vec![
                    SAMPLE_TEXT_1,
                    SAMPLE_TEXT_2,
                    SAMPLE_TEXT_3,
                ],
            });

            trim_context_from_args(
                &tokenizer,
                TOTAL_TOKENS - 7,
                TOTAL_TOKENS,
                &ContextOptions {
                    limit: None,
                    keep: OverflowKeep::Start,
                    trim_args: vec!["test".to_string()],
                    array_priority: ArrayTrimPriority::Last,
                },
                &mut args,
            )
            .unwrap();

            assert_eq!(
                args,
                object!({
                    "test": vec![
                        "This is a test texting",
                        SAMPLE_TEXT_2,
                        SAMPLE_TEXT_3,
                    ],
                })
            );

            let total_tokens = sum_tokens(&tokenizer, &args["test"]);
            assert!(total_tokens == TOTAL_TOKENS - 7);
        }

        #[test]
        fn trim_array_value_equal() {
            let tokenizer = Tokenizer::new().unwrap();
            let mut args = object!({
                "test": vec![
                    SAMPLE_TEXT_1,
                    SAMPLE_TEXT_2,
                    SAMPLE_TEXT_3,
                ],
            });

            trim_context_from_args(
                &tokenizer,
                TOTAL_TOKENS - 10,
                TOTAL_TOKENS,
                &ContextOptions {
                    limit: None,
                    keep: OverflowKeep::Start,
                    trim_args: vec!["test".to_string()],
                    array_priority: ArrayTrimPriority::Equal,
                },
                &mut args,
            )
            .unwrap();

            let total_tokens = sum_tokens(&tokenizer, &args["test"]);
            assert!(total_tokens == TOTAL_TOKENS - 10);

            assert_eq!(
                args,
                object!({
                    "test": vec![
                        "This is a test texting and",
                        "Another test text",
                        "Testing test",
                    ],
                })
            );
        }
    }
}
