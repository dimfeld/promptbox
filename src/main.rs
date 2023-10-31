use args::{parse_template_args, Args};
use clap::Parser;
use config::Config;
use error::Error;
use error_stack::{Report, ResultExt};
use template::ParsedTemplate;

mod args;
mod config;
mod error;
mod model;
mod template;
mod openai;

fn main() -> Result<(), Report<Error>> {
    let config = Config::from_directory(std::env::current_dir().unwrap())?;
    let args = Args::parse();

    let ParsedTemplate {
        template,
        path: template_path,
        input,
    } = config.find_template(&args.template)?;

    let template_context = parse_template_args(&input)?;

    let template = match (args.prepend, args.append) {
        (Some(pre), Some(post)) => format!("{pre}\n\n{template}\n\n{post}"),
        (Some(pre), None) => format!("{pre}\n\n{template}"),
        (None, Some(post)) => format!("{template}\n\n{post}"),
        (None, None) => template,
    };

    let result = tera::Tera::one_off(&template, &template_context, false)
        .change_context(Error::ParseTemplate)
        .attach_printable_lazy(|| template_path.display().to_string())?;

    if args.print_prompt || args.verbose || args.dry_run {
        println!("{}", result);
    }

    if args.dry_run {
        return Ok(());
    }

    // TODO submit it to the model

    Ok(())
}
