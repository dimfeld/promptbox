use config::Config;
use error::Error;
use error_stack::Report;

mod config;
mod error;
mod model;
mod template;

fn main() -> Result<(), Report<Error>> {
    let config = Config::from_directory(std::env::current_dir().unwrap())?;

    Ok(())
}
