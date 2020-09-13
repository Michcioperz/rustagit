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

struct CommitInfo<'a> {
    commit: git2::Commit<'a>,
    tree: git2::Tree<'a>,
    parent_tree: Option<git2::Tree<'a>>,
    diff: git2::Diff<'a>,
}

impl<'a> CommitInfo<'a> {
    fn time(&self) -> chrono::DateTime<chrono::FixedOffset> {
        use chrono::TimeZone;
        let commit_time = self.commit.time();
        let offset = chrono::FixedOffset::east(commit_time.offset_minutes() * 60);
        offset.timestamp(commit_time.seconds(), 0)
    }
}

fn commit_log<'a>(
    repository: &'a git2::Repository,
) -> Result<impl Iterator<Item = Result<CommitInfo<'a>>>> {
    let mut log_walk = repository.revwalk()?;
    log_walk.push_head()?;
    Ok(log_walk.map(move |oid_result| -> Result<_> {
        let oid = oid_result?;
        let commit = repository.find_commit(oid)?;
        let tree = commit.tree()?;
        let parent_tree = commit
            .parents()
            .next()
            .and_then(|parent| parent.tree().ok());
        let diff = repository.diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), None)?;
        Ok(CommitInfo {
            commit,
            tree,
            parent_tree,
            diff,
        })
    }))
}

fn main() -> Result<()> {
    let args: Args = argh::from_env();
    let repository = git2::Repository::open(args.source)?;
    let head = repository.head()?;
    let head_tree = head.peel_to_tree()?;
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
                @for ci_result in commit_log(&repository)? {
                    @let ci = ci_result?;
                    tr {
                        td { (ci.time().to_rfc2822()) }
                        td { (ci.commit.summary().ok_or(InvalidUtf::InvalidUtf)?) }
                        td { (ci.commit.author().name().ok_or(InvalidUtf::InvalidUtf)?) }
                        @let diffstats = ci.diff.stats()?;
                        td { (diffstats.files_changed()) }
                        td { (diffstats.insertions()) }
                        td { (diffstats.deletions()) }
                    }
                }
            }
        }
    };

    for ci_result in commit_log(&repository)? {
        let ci = ci_result?;
        let patch = html! {
            dl {
                dd { "commit" }
                dt { (ci.commit.id()) }
                @for parent in ci.commit.parents() {
                    dd { "parent" }
                    dt { (parent.id()) }
                }
                dd { "author" }
                dt {
                    (ci.commit.author().name().ok_or(InvalidUtf::InvalidUtf)?)
                    " <"
                    @let sig = ci.commit.author();
                    @let email = sig.email().ok_or(InvalidUtf::InvalidUtf)?;
                    a href={"mailto:" (&email)} { (email) }
                    ">"
                }
                dd { "committer" }
                dt {
                    (ci.commit.author().name().ok_or(InvalidUtf::InvalidUtf)?)
                    " <"
                    @let sig = ci.commit.committer();
                    @let email = sig.email().ok_or(InvalidUtf::InvalidUtf)?;
                    a href={"mailto:" (&email)} { (email) }
                    ">"
                }
                dd { "message" }
                dt {
                    pre { (ci.commit.message().ok_or(InvalidUtf::InvalidUtf)?) }
                }
                dd { "diffstat" }
                dt {
                    pre {
                        (ci.diff.stats()?.to_buf(git2::DiffStatsFormat::FULL, 72)?.as_str().ok_or(InvalidUtf::InvalidUtf)?)
                    }
                }
            }
            @for (delta_id, delta) in ci.diff.deltas().enumerate() {
                @let patch = git2::Patch::from_diff(&ci.diff, delta_id)?;
                @match patch {
                    Some(mut patch) => {
                        pre { (patch.to_buf()?.as_str().ok_or(InvalidUtf::InvalidUtf)?) }
                    }
                    None => { "unchanged or binary" }
                }
            }
        };
        dbg!(patch);
    }

    dbg!(log);

    Ok(())
}
