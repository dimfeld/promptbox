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

pub fn parse_template_args(template: &PromptTemplate) -> Result<tera::Context, Report<Error>> {
    let args = template
        .options
        .iter()
        .map(|option| {
            let action = match (option.array, option.option_type) {
                (true, _) => ArgAction::Append,
                (false, OptionType::String) => ArgAction::Set,
                (false, OptionType::Number) => ArgAction::Set,
                (false, OptionType::Integer) => ArgAction::Set,
                (false, OptionType::Bool) => ArgAction::SetTrue,
            };

            let arg = Arg::new(option.name.clone())
                .long(option.name.clone())
                .required(option.required)
                .action(action);

            let arg = match option.option_type {
                OptionType::String => {
                    arg.value_parser(clap::builder::NonEmptyStringValueParser::new())
                }
                OptionType::Number => arg.value_parser(clap::value_parser!(f32)),
                OptionType::Integer => arg.value_parser(clap::value_parser!(i64)),
                OptionType::Bool => arg.value_parser(clap::value_parser!(bool)),
            };

            arg
        })
        .collect::<Vec<_>>();

    let mut parsed = Command::new("template")
        .args(args)
        .try_get_matches()
        .change_context(Error::ArgParseFailure)?;

    let mut context = tera::Context::new();
    for option in &template.options {
        match option.option_type {
            OptionType::Bool => add_val_to_context::<bool>(&mut context, &mut parsed, option),
            OptionType::Number => add_val_to_context::<f32>(&mut context, &mut parsed, option),
            OptionType::Integer => add_val_to_context::<i64>(&mut context, &mut parsed, option),
            OptionType::String => add_val_to_context::<String>(&mut context, &mut parsed, option),
        }
    }

    Ok(context)
}

fn add_val_to_context<T: Clone + Send + Sync + Into<tera::Value> + 'static>(
    context: &mut tera::Context,
    args: &mut ArgMatches,
    option: &PromptOption,
) {
    let val = if option.array {
        if let Some(vals) = args.remove_many::<T>(&option.name) {
            let vals = vals.into_iter().map(|val| val.into()).collect();
            tera::Value::Array(vals)
        } else {
            tera::Value::Array(vec![])
        }
    } else {
        args.remove_one::<T>(&option.name)
            .map(|val| val.into())
            .unwrap_or_default()
    };

    context.insert(option.name.clone(), &val);
}
