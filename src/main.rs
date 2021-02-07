use anyhow::Result;
use argh::FromArgs;
use std::path::PathBuf;
use thiserror::Error;

pub(crate) mod repository;
pub(crate) mod templates;

/// Generate a static website presenting nicely contents of a git repository.
#[derive(FromArgs)]
struct Args {
    /// directory with git repository to process
    #[argh(positional)]
    source: PathBuf,

    /// directory to write html files into
    #[argh(positional)]
    destination: PathBuf,
}

#[derive(Error, Debug)]
#[error("invalid utf sequence")]
pub struct InvalidUtf;

fn main() -> Result<()> {
    better_panic::install();
    let args: Args = argh::from_env();
    let repository = repository::Repository::open(args.source)?;

    let syntax_set = syntect::parsing::SyntaxSet::load_defaults_newlines();
    let theme_set = syntect::highlighting::ThemeSet::load_defaults();
    let theme = &theme_set.themes["InspiredGitHub"];
    let url = templates::UrlResolver::new(args.destination.canonicalize()?);
    let templator = templates::Templator {
        repository,
        url,
        syntax_set,
        theme,
    };

    templator.generate()?;

    Ok(())
}
