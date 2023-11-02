use std::{ffi::OsString, path::PathBuf};

use args::{parse_template_args, Args};
use clap::Parser;
use config::Config;
use error::Error;
use error_stack::{Report, ResultExt};
use liquid::partials::{InMemorySource, LazyCompiler};
use template::ParsedTemplate;

mod args;
mod config;
mod error;
mod model;
mod openai;
mod template;
#[cfg(test)]
mod tests;

fn generate_template(
    args: &Args,
    base_dir: PathBuf,
    cmdline: impl IntoIterator<Item = impl Into<OsString> + Clone>,
) -> Result<String, Report<Error>> {
    let config = Config::from_directory(base_dir)?;

    let ParsedTemplate {
        template,
        path: template_path,
        input,
        ..
    } = config.find_template(&args.template)?;

    let template_context = parse_template_args(cmdline, &input)?;

    let template = match (args.prepend.as_ref(), args.append.as_ref()) {
        (Some(pre), Some(post)) => format!("{pre}\n\n{template}\n\n{post}"),
        (Some(pre), None) => format!("{pre}\n\n{template}"),
        (None, Some(post)) => format!("{template}\n\n{post}"),
        (None, None) => template,
    };

    // TODO replace InMemorySource with a custom source that can look for partials in the various
    // config directories.
    let parser = liquid::ParserBuilder::<LazyCompiler<InMemorySource>>::default()
        .stdlib()
        .build()
        .expect("failed to build parser");

    let parsed = parser
        .parse(&template)
        .change_context(Error::ParseTemplate)
        .attach_printable_lazy(|| template_path.display().to_string())?;

    parsed
        .render(&template_context)
        .change_context(Error::ParseTemplate)
        .attach_printable_lazy(|| template_path.display().to_string())
}

fn run(base_dir: PathBuf, cmdline: impl Fn() -> std::env::Args) -> Result<(), Report<Error>> {
    let args = Args::parse_from(cmdline());
    let template = generate_template(&args, base_dir, cmdline())?;

    if args.print_prompt || args.verbose || args.dry_run {
        println!("{}", template);
    }

    if args.dry_run {
        return Ok(());
    }

    // TODO submit it to the model

    Ok(())
}

fn main() -> Result<(), Report<Error>> {
    run(std::env::current_dir().unwrap(), std::env::args)
}
