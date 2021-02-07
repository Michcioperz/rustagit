use anyhow::Result;
use fs_err as fs;

pub struct CommitInfo<'a> {
    pub(crate) commit: git2::Commit<'a>,
    pub(crate) diff: git2::Diff<'a>,
}

impl CommitInfo<'_> {
    pub fn time(&self) -> chrono::DateTime<chrono::FixedOffset> {
        use chrono::TimeZone;
        let commit_time = self.commit.time();
        let offset = chrono::FixedOffset::east(commit_time.offset_minutes() * 60);
        offset.timestamp(commit_time.seconds(), 0)
    }
}

pub struct Repository {
    pub(crate) inner: git2::Repository,
    pub(crate) path: std::path::PathBuf,
}

impl Repository {
    pub fn open<S: AsRef<std::path::Path>>(path: S) -> Result<Repository> {
        Ok(Repository {
            inner: git2::Repository::open(path.as_ref())?,
            path: path.as_ref().canonicalize()?,
        })
    }

    pub fn gitdir(&self) -> &std::path::Path {
        self.inner.path()
    }

    pub fn name(&self) -> String {
        self.path.file_name().unwrap().to_string_lossy().to_string()
    }

    /// Reads a text file from a given file in .git.
    /// Returns a blank String if the file does not exist.
    pub fn read_gitdir_or_blank(&self, name: &str) -> Result<String> {
        match fs::read_to_string(self.gitdir().join(name)) {
            Ok(s) => Ok(s),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok("".to_string()),
            Err(e) => Err(e.into()),
        }
    }

    pub fn description(&self) -> Result<String> {
        self.read_gitdir_or_blank("description")
    }

    pub fn url(&self) -> Result<String> {
        self.read_gitdir_or_blank("url")
    }

    pub fn commit_log(&self) -> Result<impl Iterator<Item = Result<CommitInfo<'_>>>> {
        let mut log_walk = self.inner.revwalk()?;
        log_walk.push_head()?;
        Ok(log_walk.map(move |oid_result| -> Result<_> {
            let oid = oid_result?;
            let commit = self.inner.find_commit(oid)?;
            let tree = commit.tree()?;
            let parent_tree = commit
                .parents()
                .next()
                .and_then(|parent| parent.tree().ok());
            let diff = self
                .inner
                .diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), None)?;
            Ok(CommitInfo { commit, diff })
        }))
    }
}
