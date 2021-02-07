use std::fmt::Display;
use std::io::Write;
use std::os::unix::ffi::OsStrExt;

use crate::repository::{CommitInfo, Repository};
use crate::InvalidUtf;
use anyhow::Result;
use fs_err as fs;
use maud::html;

#[derive(Clone)]
pub struct UrlResolver {
    base: std::path::PathBuf,
}

impl UrlResolver {
    pub fn new(base: std::path::PathBuf) -> Self {
        Self { base }
    }

    fn join<P: AsRef<std::path::Path>>(&self, path: P) -> Self {
        Self {
            base: self.base.join(path),
        }
    }

    fn dot_html(&self) -> Self {
        let file_name = self.base.file_name().expect("adding .html to empty path");
        let mut file_name_bytes = file_name.as_bytes().to_vec();
        file_name_bytes.extend_from_slice(b".html");
        Self {
            base: self
                .base
                .with_file_name(std::ffi::OsStr::from_bytes(&file_name_bytes)),
        }
    }

    pub fn commit_dir(&self) -> Self {
        self.join("commit")
    }

    pub fn commit_file(&self, commit: &str) -> Self {
        self.commit_dir().join(format!("{}.html", commit))
    }

    pub fn commit_log(&self) -> Self {
        self.join("log.html")
    }

    pub fn tree_dir(&self) -> Self {
        self.join("tree")
    }

    pub fn tree_index(&self) -> Self {
        self.tree_dir().dot_html()
    }

    pub fn tree_file(&self, name: &str) -> Self {
        self.tree_dir().join(name).dot_html()
    }

    pub fn refs_list(&self) -> Self {
        self.join("refs.html")
    }

    pub fn style_css(&self) -> Self {
        self.join("rustagit.css")
    }

    pub fn rel_root_from<P: AsRef<std::path::Path>>(&self, path: P) -> Self {
        let relpath = path.as_ref().strip_prefix(&self.base).unwrap();
        let exitus = "../".repeat(relpath.components().count().saturating_sub(1));
        UrlResolver {
            base: if exitus.is_empty() {
                std::path::PathBuf::from(".")
            } else {
                std::path::PathBuf::from(exitus.get(..exitus.len() - 1).unwrap())
            },
        }
    }
}

impl AsRef<std::path::Path> for UrlResolver {
    fn as_ref(&self) -> &std::path::Path {
        self.base.as_ref()
    }
}

impl Display for UrlResolver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.base.to_string_lossy())
    }
}

pub struct Templator<'a> {
    pub(crate) repository: Repository,
    pub(crate) url: UrlResolver,
    pub(crate) syntax_set: syntect::parsing::SyntaxSet,
    pub(crate) theme: &'a syntect::highlighting::Theme,
}

impl Templator<'_> {
    const DEFAULT_CSS: &'static str = r#"
        .numeric {
            text-align: right;
        }
        td.numeric {
            font-family: monospace;
        }
    "#;

    fn write_default_css_if_not_exists(&self) -> Result<()> {
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(self.url.style_css().base)
        {
            Ok(mut f) => Ok(f.write_all(Self::DEFAULT_CSS.as_bytes())?),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => Ok(()),
            Err(e) => Err(e.into()),
        }
    }

    fn template_page<P: AsRef<std::path::Path>>(
        &self,
        title: &str,
        path: P,
        content: maud::Markup,
    ) -> Result<maud::Markup> {
        let the_way_out = self.url.rel_root_from(path);
        Ok(html! {
            (maud::DOCTYPE)
            html {
                head {
                    meta charset="utf-8";
                    meta name="viewport" content="width=device-width";
                    title { (title) " – " (self.repository.name()) }
                    link rel="stylesheet" href=(the_way_out.style_css());
                }
                body {
                    nav {
                        h1 { (self.repository.name()) }
                        @let description = self.repository.description()?;
                        @if !description.is_empty() { p { (description) } }
                        @let url = self.repository.url()?;
                        @if !url.is_empty() { pre {
                            "git clone "
                            a href={(url)} { (url) }
                        } }
                        ul.inline {
                            li { a href=(the_way_out.commit_log()) { "Commits" } }
                            li { a href=(the_way_out.tree_index()) { "Files" } }
                            li { a href=(the_way_out.refs_list()) { "Branches and tags" } }
                        }
                    }
                    main { (content) }
                    footer {
                        "Powered by "
                        a href="https://git.hinata.iscute.ovh/rustagit/" {
                            "Rustagit, static git browser generator"
                        }
                    }
                }
            }
        })
    }

    fn precreate_dirs(&self) -> Result<()> {
        fs::create_dir_all(&self.url.base)?;
        fs::create_dir_all(self.url.commit_dir().base)?;
        Ok(())
    }

    fn write_commit_log(&self) -> Result<()> {
        let log_path = self.url.commit_log();
        let log = self.template_page(
            "Commit log",
            &log_path.base,
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
                        @for ci_result in self.repository.commit_log()? {
                            @let ci = ci_result?;
                            tr {
                                td {
                                    abbr title={(ci.time())} {
                                        (ci.time().date().format("%Y-%m-%d"))
                                    }
                                }
                                td {
                                    a href={"commit/" (ci.commit.id()) ".html"} {
                                        (ci.commit.summary().ok_or(InvalidUtf)?)
                                    }
                                }
                                td { (ci.commit.author().name().ok_or(InvalidUtf)?) }
                                @let diffstats = ci.diff.stats()?;
                                td.numeric { (diffstats.files_changed()) }
                                td.numeric { (diffstats.insertions()) }
                                td.numeric { (diffstats.deletions()) }
                            }
                        }
                    }
                }
            },
        )?;
        fs::write(log_path.base, log.into_string().as_bytes())?;
        Ok(())
    }

    pub fn write_commit(&self, ci: &CommitInfo) -> Result<()> {
        let patch_path = self.url.commit_file(&ci.commit.id().to_string());
        let patch = self.template_page(
            &format!("Commit {}", ci.commit.id()),
            &patch_path.base,
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
                        (ci.commit.author().name().ok_or(InvalidUtf)?)
                        " <"
                        @let sig = ci.commit.author();
                        @let email = sig.email().ok_or(InvalidUtf)?;
                        a href={"mailto:" (&email)} { (email) }
                        ">"
                    }
                    dt { "committer" }
                    dd {
                        (ci.commit.author().name().ok_or(InvalidUtf)?)
                        " <"
                        @let sig = ci.commit.committer();
                        @let email = sig.email().ok_or(InvalidUtf)?;
                        a href={"mailto:" (&email)} { (email) }
                        ">"
                    }
                    dt { "message" }
                    dd {
                        pre { (ci.commit.message().ok_or(InvalidUtf)?) }
                    }
                    dt { "diffstat" }
                    dd {
                        pre {
                            (ci.diff.stats()?.to_buf(git2::DiffStatsFormat::FULL, 72)?.as_str().ok_or(InvalidUtf)?)
                        }
                    }
                }
                @for (delta_id, _delta) in ci.diff.deltas().enumerate() {
                    @let patch = git2::Patch::from_diff(&ci.diff, delta_id)?;
                    @match patch {
                        Some(mut patch) => {
                            pre { (patch.to_buf()?.as_str().ok_or(InvalidUtf)?) }
                        }
                        None => { "unchanged or binary" }
                    }
                }
            },
        )?;
        fs::write(patch_path.base, patch.into_string().as_bytes())?;
        Ok(())
    }

    pub fn write_all_commits(&self) -> Result<()> {
        for ci_result in self.repository.commit_log()? {
            self.write_commit(&ci_result?)?;
        }
        Ok(())
    }

    pub fn write_tree_branch<'a, T: Iterator<Item = git2::TreeEntry<'a>>>(
        &self,
        subtree: T,
        file_path: UrlResolver,
        tree_path: std::path::PathBuf,
    ) -> Result<()> {
        fs::create_dir_all(file_path.base.parent().unwrap())?;
        let subtree_root = UrlResolver {
            base: std::path::PathBuf::from(file_path.base.with_extension("").file_name().unwrap()),
        };
        let content = self.template_page(
            tree_path.to_str().ok_or(InvalidUtf)?,
            &file_path,
            html! {
                ul {
                    @for item in subtree {
                        li {
                            @let name = item.name().ok_or(InvalidUtf)?;
                            a href=(subtree_root.join(name).dot_html()) {
                                @if let Some(git2::ObjectType::Tree) = item.kind() {
                                    (name) "/"
                                } @else {
                                    (name)
                                }
                            }
                        }
                    }
                }
            },
        )?;
        fs::write(file_path.base, content.into_string().as_bytes())?;
        Ok(())
    }

    fn highlight_object<P: AsRef<std::path::Path>>(
        &self,
        output_path: P,
        content: &str,
    ) -> Result<maud::Markup> {
        let file_name = output_path
            .as_ref()
            .file_name()
            .unwrap()
            .to_str()
            .ok_or(InvalidUtf)?;
        let name_syntax = self.syntax_set.find_syntax_by_extension(&file_name);
        let ext_syntax = self.syntax_set.find_syntax_by_extension(
            output_path
                .as_ref()
                .extension()
                .and_then(|x| x.to_str())
                .unwrap_or_default(),
        );
        let first_line = syntect::util::LinesWithEndings::from(content)
            .next()
            .unwrap_or_default();
        let line_syntax = self.syntax_set.find_syntax_by_first_line(first_line);
        let syntax = name_syntax
            .or(ext_syntax)
            .or(line_syntax)
            .unwrap_or_else(|| self.syntax_set.find_syntax_plain_text());
        Ok(maud::PreEscaped(
            syntect::html::highlighted_html_for_string(
                content,
                &self.syntax_set,
                syntax,
                self.theme,
            ),
        ))
    }

    pub fn write_tree_leaf(
        &self,
        object: git2::Blob,
        file_path: UrlResolver,
        tree_path: std::path::PathBuf,
    ) -> Result<()> {
        let raw_name = file_path.base.with_extension("");
        let content = self.template_page(tree_path.to_str().ok_or(InvalidUtf)?, &file_path, match std::str::from_utf8(object.content()) {
            Ok(content) => {
                self.highlight_object(raw_name, content)?
            },
            Err(_) => {
                fs::write(&raw_name, object.content())?;
                html! {
                    p { "This is not a file of UTF-8 honour." }
                    a href=(raw_name.file_name().unwrap().to_str().ok_or(InvalidUtf)?) { "See raw" }
                }
            },
        })?;
        fs::write(file_path.base, content.into_string().as_bytes())?;
        Ok(())
    }

    pub fn write_all_tree_nodes(&self) -> Result<()> {
        let head = self.repository.inner.head()?;
        let head_tree = head.peel_to_tree()?;
        let mut err = None;
        let tree_root = self.url.tree_dir();
        let slash_root = std::path::PathBuf::from("/");
        let walker = |parent: &str, entry: &git2::TreeEntry| -> Result<()> {
            let output_path = if !parent.is_empty() {
                tree_root.join(parent)
            } else {
                tree_root.clone()
            }
            .join(entry.name().ok_or(InvalidUtf)?)
            .dot_html();
            let subtree_path = if !parent.is_empty() {
                slash_root.join(parent)
            } else {
                slash_root.clone()
            }
            .join(entry.name().ok_or(InvalidUtf)?);
            match entry.kind() {
                Some(git2::ObjectType::Tree) => {
                    let subtree = entry.to_object(&self.repository.inner)?.peel_to_tree()?;
                    self.write_tree_branch(subtree.into_iter(), output_path, subtree_path)?;
                }
                Some(git2::ObjectType::Blob) => {
                    let obj = entry.to_object(&self.repository.inner)?.peel_to_blob()?;
                    self.write_tree_leaf(obj, output_path, subtree_path)?;
                }
                _ => {}
            }
            Ok(())
        };
        self.write_tree_branch(
            head_tree.into_iter(),
            tree_root.dot_html(),
            std::path::PathBuf::from("/"),
        )?;
        head_tree
            .walk(git2::TreeWalkMode::PreOrder, |parent, entry| {
                match walker(parent, entry) {
                    Ok(()) => git2::TreeWalkResult::Ok,
                    Err(e) => {
                        err = Some(e);
                        git2::TreeWalkResult::Abort
                    }
                }
            })
            .map_err(|e| -> anyhow::Error {
                if e.class() == git2::ErrorClass::Callback {
                    err.unwrap().into()
                } else {
                    e.into()
                }
            })?;
        Ok(())
    }

    pub fn generate(&self) -> Result<()> {
        self.precreate_dirs()?;
        self.write_default_css_if_not_exists()?;
        self.write_commit_log()?;
        self.write_all_commits()?;
        self.write_all_tree_nodes()?;
        Ok(())
    }
}
