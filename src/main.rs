use anyhow::Result;
use argh::FromArgs;
use std::path::PathBuf;

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

fn main() -> Result<()> {
    let args: Args = argh::from_env();
    let repository = git2::Repository::open(args.source)?;
    let head_tree = repository.head()?.peel_to_tree()?;
    for entry in head_tree.iter() {
        dbg!(entry.name());
    }
    Ok(())
}
