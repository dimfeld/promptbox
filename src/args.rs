use std::path::PathBuf;

use clap::{Arg, ArgAction, ArgMatches, Command, Parser};
use error_stack::{Report, ResultExt};

use crate::{
    error::Error,
    template::{OptionType, PromptOption, PromptTemplate},
};

#[derive(Parser, Debug)]
pub struct Args {
    /// The name of the template to read
    pub template: String,

    /// Override the model used by the template
    #[arg(long, short = 'm')]
    pub model: Option<String>,

    /// Override the temperature value passed to the model
    #[arg(long, short = 't')]
    pub temperature: Option<f32>,

    /// Prepend this text to the template
    #[arg(long = "pre")]
    pub prepend: Option<String>,

    /// Append this text to the template
    #[arg(long = "post")]
    pub append: Option<String>,

    /// Print the generated prompt
    #[arg(long)]
    pub print_prompt: bool,

    /// Print the generated prompt and exit without submitting it to the model
    #[arg(long)]
    pub dry_run: bool,

    /// Print the prompt and the model parameters
    #[arg(long, short)]
    pub verbose: bool,
    // /// Output JSON instead of just text
    // #[arg(long)]
    // pub json: bool,
}

pub fn parse_template_args(template: &PromptTemplate) -> Result<liquid::Object, Report<Error>> {
    let args = template
        .options
        .iter()
        .map(|(name, option)| {
            let action = match (option.array, option.option_type) {
                (true, _) => ArgAction::Append,
                (false, OptionType::String) => ArgAction::Set,
                (false, OptionType::Number) => ArgAction::Set,
                (false, OptionType::Integer) => ArgAction::Set,
                (false, OptionType::File) => ArgAction::Set,
                (false, OptionType::Bool) => ArgAction::SetTrue,
            };

            let arg = Arg::new(name.to_string())
                .long(name.to_string())
                .required(!option.optional)
                .action(action);

            let arg = match option.option_type {
                OptionType::String => {
                    arg.value_parser(clap::builder::NonEmptyStringValueParser::new())
                }
                OptionType::Number => arg.value_parser(clap::value_parser!(f32)),
                OptionType::Integer => arg.value_parser(clap::value_parser!(i64)),
                OptionType::Bool => arg.value_parser(clap::value_parser!(bool)),
                OptionType::File => arg.value_parser(clap::value_parser!(PathBuf)),
            };

            arg
        })
        .collect::<Vec<_>>();

    let mut parsed = Command::new("template")
        .args(args)
        .try_get_matches()
        .change_context(Error::ArgParseFailure)?;

    let mut context = liquid::Object::new();
    for (name, option) in &template.options {
        match option.option_type {
            OptionType::Bool => add_val_to_context::<bool>(&mut context, &mut parsed, name, option),
            OptionType::Number => {
                add_val_to_context::<f32>(&mut context, &mut parsed, name, option)
            }
            OptionType::Integer => {
                add_val_to_context::<i64>(&mut context, &mut parsed, name, option)
            }
            OptionType::String => {
                add_val_to_context::<String>(&mut context, &mut parsed, name, option)
            }
            OptionType::File => {
                if option.array {
                    let vals = parsed.remove_many::<PathBuf>(&name).unwrap_or_default();
                    let values = vals
                        .into_iter()
                        .map(|val| {
                            let contents =
                                std::fs::read_to_string(&val).attach_printable_lazy(|| {
                                    format!("Could not read file: {}", val.display().to_string())
                                })?;

                            Ok::<_, Report<std::io::Error>>(liquid::model::Value::scalar(contents))
                        })
                        .collect::<Result<Vec<_>, _>>()
                        .change_context(Error::ArgParseFailure)?;
                    context.insert(name.into(), liquid::model::Value::Array(values));
                } else {
                    let val = parsed
                        .remove_one::<PathBuf>(name)
                        .map(|val| {
                            std::fs::read_to_string(&val)
                                .attach_printable_lazy(|| {
                                    format!("Could not read file: {}", val.display().to_string())
                                })
                                .map(liquid::model::Value::scalar)
                        })
                        .transpose()
                        .change_context(Error::ArgParseFailure)?;
                    context.insert(name.into(), val.unwrap_or(liquid::model::Value::Nil));
                }
            }
        }
    }

    Ok(context)
}

fn add_val_to_context<
    T: Clone + Send + Sync + Into<liquid::model::ScalarCow<'static>> + 'static,
>(
    context: &mut liquid::Object,
    args: &mut ArgMatches,
    name: &str,
    option: &PromptOption,
) {
    let val = if option.array {
        if let Some(vals) = args.remove_many::<T>(name) {
            let vals = vals
                .into_iter()
                .map(|val| liquid::model::Value::scalar(val))
                .collect();
            liquid::model::Value::Array(vals)
        } else {
            liquid::model::Value::Array(vec![])
        }
    } else {
        args.remove_one::<T>(&option.name)
            .map(liquid::model::Value::scalar)
            .unwrap_or(liquid::model::Value::Nil)
    };

    context.insert(option.name.clone().into(), val);
}
