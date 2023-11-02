use std::{ffi::OsString, path::PathBuf};

use clap::{Arg, ArgAction, ArgMatches, Command, Parser};
use error_stack::{Report, ResultExt};

use crate::{
    error::Error,
    template::{OptionType, PromptOption, PromptTemplate},
};

#[derive(Parser, Debug, Default)]
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

pub fn parse_template_args(
    cmdline: impl IntoIterator<Item = impl Into<OsString> + Clone>,
    template: &PromptTemplate,
) -> Result<liquid::Object, Report<Error>> {
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
                .required(option.default.is_none() && !option.optional)
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

            Ok(arg)
        })
        .collect::<Result<Vec<_>, Report<Error>>>()?;

    let mut parsed = Command::new("template")
        .args(args)
        .try_get_matches_from(cmdline)
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
                        .map(create_file_object)
                        .collect::<Result<Vec<_>, _>>()
                        .change_context(Error::ArgParseFailure)?;
                    context.insert(name.into(), liquid::model::Value::Array(values));
                } else {
                    let val = parsed
                        .remove_one::<PathBuf>(name)
                        .map(create_file_object)
                        .transpose()
                        .change_context(Error::ArgParseFailure)?;
                    context.insert(name.into(), val.unwrap_or(liquid::model::Value::Nil));
                }
            }
        }
    }

    Ok(context)
}

fn create_file_object(path: PathBuf) -> Result<liquid::model::Value, Report<std::io::Error>> {
    let contents = std::fs::read_to_string(&path)
        .attach_printable_lazy(|| format!("Could not read file: {}", path.display()))?;

    let obj = liquid::object!({
        "filename": path.file_name(),
        "path": path.to_string_lossy().to_owned(),
        "contents": contents
    });

    Ok(liquid::model::Value::Object(obj))
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
            option
                .default
                .clone()
                .unwrap_or_else(|| liquid::model::Value::array(vec![]))
        }
    } else {
        args.remove_one::<T>(name)
            .map(liquid::model::Value::scalar)
            .or_else(|| option.default.clone())
            .unwrap_or(liquid::model::Value::Nil)
    };

    context.insert(name.to_string().into(), val);
}
