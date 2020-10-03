use anyhow::Result;
use argh::FromArgs;
use fs_err as fs;
use maud::html;
use std::io::Write;
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

    #[inline]
    fn simplify_formatted_duration(duration: humantime::FormattedDuration) -> String {
        let duration = format!("{}", duration);
        duration
            .as_str()
            .split_whitespace()
            .take(2)
            .collect::<Vec<_>>()
            .join(" ")
    }

    fn human_time(&self, now: chrono::DateTime<chrono::Local>) -> String {
        let duration = self.time().signed_duration_since(now);
        if duration < chrono::Duration::zero() {
            format!(
                "{} ago",
                Self::simplify_formatted_duration(humantime::format_duration(
                    (-duration)
                        .to_std()
                        .expect("out of range duration when converting from chrono to std")
                ))
            )
        } else {
            format!(
                "in {}",
                Self::simplify_formatted_duration(humantime::format_duration(
                    duration
                        .to_std()
                        .expect("out of range duration when converting from chrono to std")
                ))
            )
        }
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

fn page_func(
    name: String,
    description: String,
    url: String,
    base_path: PathBuf,
) -> Box<dyn Fn(&str, &PathBuf, maud::Markup) -> maud::Markup> {
    Box::new(move |title: &str, path: &PathBuf, content: maud::Markup| {
        let relpath = path.strip_prefix(&base_path).unwrap();
        let the_way_out = "../".repeat(relpath.components().count().saturating_sub(1));
        html! {
            (maud::DOCTYPE)
            html {
                head {
                    meta charset="utf-8";
                    meta name="viewport" content="width=device-width";
                    title { (title) " – " (name) }
                }
                body {
                    nav {
                        h1 { (name) }
                        p { (description) }
                        pre {
                            "git clone "
                            a href={(url)} { (url) }
                        }
                        ul.inline {
                            // TODO: relativize links
                            li { a href={(the_way_out) "log.html"} { "Commits" } }
                            li { a href={(the_way_out) "tree.html"} { "Files" } }
                            li { a href={(the_way_out) "refs.html"} { "Branches and tags" } }
                        }
                    }
                    main { (content) }
                }
            }
        }
    })
}

fn main() -> Result<()> {
    let args: Args = argh::from_env();
    let repository = git2::Repository::open(args.source)?;
    let head = repository.head()?;
    let head_tree = head.peel_to_tree()?;

    let repository_name = "TODO_repository_name".to_string();
    let repository_description = "TODO_repository_description".to_string();
    let repository_url = "https://git.hinata.iscute.ovh/TODO".to_string();
    let page = page_func(
        repository_name,
        repository_description,
        repository_url,
        args.destination.clone(),
    );

    let syntax_set = syntect::parsing::SyntaxSet::load_defaults_newlines();
    let theme_set = syntect::highlighting::ThemeSet::load_defaults();
    let theme = &theme_set.themes["InspiredGitHub"];

    fs::create_dir_all(&args.destination)?;

    let log_path = &args.destination.join("log.html");
    let log = page(
        "Commit log",
        &log_path,
        html! {
            table {
                thead {
                    tr {
                        th { "Date" }
                        th { "Commit message" }
                        th { "Author" }
                        th.numeric { "Files" }
                        th.numeric { "+" }
                        th.numeric { "-" }
                    }
                }
                tbody {
                    @for ci_result in commit_log(&repository)? {
                        @let ci = ci_result?;
                        tr {
                            td {
                                abbr title={(ci.time())} {
                                    (ci.time().date().format("%Y-%m-%d"))
                                }
                            }
                            td {
                                a href={"commit/" (ci.commit.id()) ".html"} {
                                    (ci.commit.summary().ok_or(InvalidUtf::InvalidUtf)?)
                                }
                            }
                            td { (ci.commit.author().name().ok_or(InvalidUtf::InvalidUtf)?) }
                            @let diffstats = ci.diff.stats()?;
                            td.numeric { (diffstats.files_changed()) }
                            td.numeric { (diffstats.insertions()) }
                            td.numeric { (diffstats.deletions()) }
                        }
                    }
                }
            }
        },
    );
    fs::File::create(log_path).and_then(|mut f| f.write_all(log.into_string().as_bytes()))?;

    let commits_dir = args.destination.join("commit");
    fs::create_dir_all(&commits_dir)?;
    for ci_result in commit_log(&repository)? {
        let ci = ci_result?;
        let patch_path = commits_dir.join(format!("{}.html", ci.commit.id()));
        let patch = page(
            &format!("Commit {}", ci.commit.id()),
            &patch_path,
            html! {
                dl {
                    dt { "commit" }
                    dd { (ci.commit.id()) }
                    @for parent in ci.commit.parents() {
                        dt { "parent" }
                        dd { a href={(parent.id()) ".html"} { (parent.id()) } }
                    }
                    dt { "author" }
                    dd {
                        (ci.commit.author().name().ok_or(InvalidUtf::InvalidUtf)?)
                        " <"
                        @let sig = ci.commit.author();
                        @let email = sig.email().ok_or(InvalidUtf::InvalidUtf)?;
                        a href={"mailto:" (&email)} { (email) }
                        ">"
                    }
                    dt { "committer" }
                    dd {
                        (ci.commit.author().name().ok_or(InvalidUtf::InvalidUtf)?)
                        " <"
                        @let sig = ci.commit.committer();
                        @let email = sig.email().ok_or(InvalidUtf::InvalidUtf)?;
                        a href={"mailto:" (&email)} { (email) }
                        ">"
                    }
                    dt { "message" }
                    dd {
                        pre { (ci.commit.message().ok_or(InvalidUtf::InvalidUtf)?) }
                    }
                    dt { "diffstat" }
                    dd {
                        pre {
                            (ci.diff.stats()?.to_buf(git2::DiffStatsFormat::FULL, 72)?.as_str().ok_or(InvalidUtf::InvalidUtf)?)
                        }
                    }
                }
                @for (delta_id, _delta) in ci.diff.deltas().enumerate() {
                    @let patch = git2::Patch::from_diff(&ci.diff, delta_id)?;
                    @match patch {
                        Some(mut patch) => {
                            pre { (patch.to_buf()?.as_str().ok_or(InvalidUtf::InvalidUtf)?) }
                        }
                        None => { "unchanged or binary" }
                    }
                }
            },
        );
        fs::File::create(patch_path)
            .and_then(|mut f| f.write_all(patch.into_string().as_bytes()))?;
    }

    let tree_root = args.destination.join("tree");
    fs::create_dir_all(&tree_root)?;
    head_tree.walk(git2::TreeWalkMode::PreOrder, |parent, entry| {
        // dbg!((entry.name(), entry.kind()));
        let parent_path = if parent.len() > 0 {
            tree_root.join(parent)
        } else {
            tree_root.clone()
        };
        let full_filename = format!("{}{}", parent, entry.name().unwrap());
        match entry.kind() {
            Some(git2::ObjectType::Tree) => {
                let path = parent_path.join(entry.name().unwrap());
                fs::create_dir_all(&path).unwrap();
                let subtree = entry
                    .to_object(&repository)
                    .unwrap()
                    .peel_to_tree()
                    .unwrap();
                let subtree_path = path.join("index.html");
                let subtree_html = page(
                    &full_filename,
                    &subtree_path,
                    html! {
                        ul {
                            @for item in subtree.iter() {
                                li {
                                    a href={(item.name().unwrap()) ".html"} {
                                        (item.name().unwrap())
                                    }
                                }
                            }
                        }
                    },
                );
                fs::File::create(subtree_path)
                    .and_then(|mut f| f.write_all(subtree_html.into_string().as_bytes()))
                    .unwrap();
            }
            Some(git2::ObjectType::Blob) => {
                let path = parent_path.join(format!("{}.html", entry.name().unwrap()));
                let obj = entry
                    .to_object(&repository)
                    .unwrap()
                    .peel_to_blob()
                    .unwrap();
                let content = std::str::from_utf8(obj.content()).unwrap();
                let name = entry.name().unwrap();
                let filename = PathBuf::from(&name);
                let name_syntax = syntax_set.find_syntax_by_extension(&name);
                let ext_syntax = syntax_set.find_syntax_by_extension(
                    filename.extension().and_then(|x| x.to_str()).unwrap_or(""),
                );
                let first_line = syntect::util::LinesWithEndings::from(&content)
                    .next()
                    .unwrap_or_default();
                let line_syntax = syntax_set.find_syntax_by_first_line(first_line);
                let syntax = name_syntax
                    .or(ext_syntax)
                    .or(line_syntax)
                    .unwrap_or_else(|| syntax_set.find_syntax_plain_text());
                let snippet = maud::PreEscaped(syntect::html::highlighted_html_for_string(
                    content,
                    &syntax_set,
                    syntax,
                    theme,
                ));
                let blob = page(
                    &full_filename,
                    &path,
                    html! {
                        (snippet)
                    },
                );
                fs::File::create(path)
                    .and_then(|mut f| f.write_all(blob.into_string().as_bytes()))
                    .unwrap();
            }
            _ => {}
        }
        git2::TreeWalkResult::Ok
    })?;

    Ok(())
}
