use anyhow::Result;
use argh::FromArgs;
use maud::html;
use std::path::PathBuf;
use thiserror::Error;

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
enum InvalidUtf {
    #[error("invalid utf sequence")]
    InvalidUtf,
}

fn main() -> Result<()> {
    let args: Args = argh::from_env();
    let repository = git2::Repository::open(args.source)?;
    let head = repository.head()?;
    let head_tree = head.peel_to_tree()?;
    let mut log_walk = repository.revwalk()?;
    log_walk.push_head()?;
    let commits: Result<Vec<_>> = log_walk.map(|oid_result| {
        let oid = oid_result?;
        let commit = repository.find_commit(oid)?;
        let tree = commit.tree()?;
        let parent_tree = commit.parents().next().and_then(|parent| parent.tree().ok());
        let diff = repository.diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), None)?;
        Ok(commit)
    }).collect();
    let commits = commits?;
    let log = html! {
        table {
            thead {
                tr {
                    th { "Date" }
                    th { "Commit message" }
                    th { "Author" }
                    th { "Files" }
                    th { "+" }
                    th { "-" }
                }
            }
            tbody {
                @for commit in commits {
                    tr {
                        td { (commit.time().seconds()) }
                        td { (commit.summary().ok_or(InvalidUtf::InvalidUtf)?) }
                        td { (commit.author().name().ok_or(InvalidUtf::InvalidUtf)?) }
                        td { "TODO" }
                        td { "TODO" }
                        td { "TODO" }
                    }
                }
            }
        }
    };

    dbg!(log);

    Ok(())
}
