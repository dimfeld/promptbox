use std::{
    ffi::OsString,
    path::{Path, PathBuf},
};

use clap::{
    Arg, ArgAction, ArgMatches, Command, CommandFactory, FromArgMatches, Parser, Subcommand,
};
use error_stack::{Report, ResultExt};

use crate::{
    context::OverflowKeep,
    error::Error,
    model::OutputFormat,
    template::{OptionType, PromptOption, PromptTemplate},
};

#[derive(Parser, Debug)]
pub struct Cli {
    #[command(subcommand)]
    command: MainCommand,
}

#[derive(Subcommand, Debug)]
pub enum MainCommand {
    Run(GlobalRunArgs),
    // List
    // Show
}

#[derive(Parser, Debug, Default)]
pub struct GlobalRunArgs {
    /// The template to run
    pub template: String,

    /// LM Studio host, if different from the default
    #[arg(long, env = "LM_STUDIO_HOST")]
    pub lm_studio_host: Option<String>,

    /// Ollama host, if different from the default
    #[arg(long, env = "OLLAMA_HOST")]
    pub ollama_host: Option<String>,

    /// OpenAI Key
    #[arg(long, env = "OPENAI_KEY")]
    pub openai_key: Option<String>,

    /// Override the model used by the template
    #[arg(long, short = 'm', env = "MODEL")]
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

    /// Output JSON instead of just text
    #[arg(long)]
    pub format: Option<OutputFormat>,

    /// Set which side of the context to keep when overflowing.
    /// Defaults to keeping the start.
    #[arg(long)]
    pub overflow_keep: Option<OverflowKeep>,

    /// Set a lower context size limit for a model.
    #[arg(long)]
    pub context_limit: Option<usize>,

    /// Make sure that the prompt is short enough to allow this many tokens to be generated.
    /// Default is 256.
    #[arg(long)]
    pub reserve_output_context: Option<usize>,

    /// Extra strings to add to the end of the prompt.
    pub extra_prompt: Vec<String>,
}

pub enum FoundCommand {
    Run {
        template: String,
        args: Vec<OsString>,
    },
    Other(Cli),
}

pub fn parse_main_args(cmdline: Vec<OsString>) -> Result<FoundCommand, clap::Error> {
    let first_arg = cmdline
        .get(1)
        .map(|s| s.to_string_lossy())
        .unwrap_or_default();
    let second_arg = cmdline
        .get(2)
        .map(|s| s.to_string_lossy())
        .unwrap_or_default();
    if cmdline.len() >= 3
        && first_arg == "run"
        && !second_arg.is_empty()
        && !second_arg.starts_with("-")
    {
        // This isn't great since it hardcodes looking for a specific format. Probably better to
        // use a real parse with TrailingArgs.
        Ok(FoundCommand::Run {
            template: second_arg.to_string(),
            args: cmdline,
        })
    } else {
        Cli::try_parse_from(cmdline).map(FoundCommand::Other)
    }
}

pub fn parse_template_args(
    cmdline: Vec<OsString>,
    base_dir: &Path,
    template: &PromptTemplate,
) -> Result<(GlobalRunArgs, liquid::Object), Report<Error>> {
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
                .required(
                    option.option_type != OptionType::Bool
                        && option.default.is_none()
                        && !option.optional,
                )
                .help(&option.description)
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

    // Merge together the args from the global run options and from the template.
    let run_command = Command::new("run")
        .args(GlobalRunArgs::command().get_arguments())
        .args(args);

    let main_parsed = Command::new("promptbox")
        .subcommand(run_command)
        .try_get_matches_from(cmdline)
        .map_err(Error::from)?;

    let mut parsed = main_parsed
        .subcommand_matches("run")
        .cloned()
        .ok_or(Error::ArgParseFailure)?;

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
                        .map(|path| create_file_object(base_dir, &path))
                        .collect::<Result<Vec<_>, _>>()
                        .change_context(Error::ArgParseFailure)?;
                    context.insert(name.into(), liquid::model::Value::Array(values));
                } else {
                    let val = parsed
                        .remove_one::<PathBuf>(name)
                        .map(|path| create_file_object(base_dir, &path))
                        .transpose()
                        .change_context(Error::ArgParseFailure)?;
                    context.insert(name.into(), val.unwrap_or(liquid::model::Value::Nil));
                }
            }
        }
    }

    let global_args =
        GlobalRunArgs::from_arg_matches_mut(&mut parsed).change_context(Error::ArgParseFailure)?;

    Ok((global_args, context))
}

fn create_file_object(
    base_dir: &Path,
    path: &Path,
) -> Result<liquid::model::Value, Report<std::io::Error>> {
    let contents = std::fs::read_to_string(base_dir.join(path).canonicalize()?)
        .attach_printable_lazy(|| format!("Could not read file: {}", path.display()))?;

    let obj = liquid::object!({
        "filename": path.file_name().map(|s| s.to_string_lossy()).unwrap_or_default(),
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
