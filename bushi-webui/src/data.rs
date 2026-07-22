use async_trait::async_trait;

use crate::error::DomainError;

pub mod git;
pub mod sqlite;

pub use git::GitRepository;
pub use sqlite::SqliteRepository;

#[derive(Clone, Debug)]
pub struct RepositoryRecord {
    pub id: i64,
    pub name: String,
    pub path: String,
    pub head: Option<String>,
}

#[derive(Clone, Debug)]
pub struct ResolvedRev {
    pub input: String,
    pub commit_hash: String,
}

#[derive(Clone, Debug)]
pub struct RefRecord {
    pub show_name: String,
    pub commit_hash: String,
}

#[async_trait]
pub trait RepositoryPort: Send + Sync + 'static {
    async fn list_repositories(&self) -> Result<Vec<RepositoryRecord>, DomainError>;
    async fn get_repository(&self, name: &str) -> Result<Option<RepositoryRecord>, DomainError>;
    async fn default_rev(&self, repo: &RepositoryRecord) -> Result<String, DomainError>;
    async fn resolve_rev(&self, repo_id: i64, rev: &str) -> Result<ResolvedRev, DomainError>;
    async fn ref_time(&self, repo_id: i64, rev: &str) -> Result<Option<i64>, DomainError>;
    async fn list_refs(&self, repo_id: i64, ref_type: i64) -> Result<Vec<RefRecord>, DomainError>;

    async fn log(
        &self,
        repo_id: i64,
        start_commit_hash: &str,
        after: Option<&str>,
        limit: usize,
    ) -> Result<HistoryPage, DomainError>;

    async fn path_history(
        &self,
        repo_id: i64,
        path: &str,
        start_commit_hash: &str,
        after: Option<&str>,
        limit: usize,
    ) -> Result<HistoryPage, DomainError>;

    async fn latest_change(
        &self,
        repo_id: i64,
        start_commit_hash: &str,
        path: &str,
    ) -> Result<Option<String>, DomainError>;

    async fn path_kind(&self, path: &str) -> Result<Option<bool>, DomainError>;
}

#[async_trait]
pub trait GitPort: Send + Sync + 'static {
    async fn read_readme(
        &self,
        repo_path: String,
        hash: String,
    ) -> Result<Option<ReadmeView>, DomainError>;
    async fn list_tree(
        &self,
        repo_path: String,
        hash: String,
        path: String,
    ) -> Result<Vec<TreeEntryView>, DomainError>;
    async fn read_blob(
        &self,
        repo_path: String,
        hash: String,
        path: String,
    ) -> Result<BlobView, DomainError>;
    async fn commit_diff(
        &self,
        repo_path: String,
        hash: String,
    ) -> Result<CommitDiffView, DomainError>;
    async fn read_commits(
        &self,
        repo_path: String,
        hashes: Vec<String>,
    ) -> Result<Vec<CommitInfo>, DomainError>;
}

pub fn short_hash(hash: &str) -> String {
    hash.chars().take(8).collect()
}

#[derive(Clone, Debug)]
pub struct ReadmeView {
    pub path: String,
    pub html: String,
}

#[derive(Clone, Debug)]
pub struct TreeEntryView {
    pub name: String,
    pub mode: String,
    pub kind: EntryKind,
    pub size: Option<u64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntryKind {
    Tree,
    Blob,
    Commit,
    Other,
}

#[derive(Clone, Debug)]
pub struct BlobView {
    pub bytes: Vec<u8>,
    pub size: usize,
    pub text_lines: Option<Vec<String>>,
}

#[derive(Clone, Debug)]
pub struct CommitInfo {
    pub hash: String,
    pub short_hash: String,
    pub subject: String,
    pub message: String,
    pub author: String,
    pub author_email: String,
    pub time_label: String,
    pub parent_hashes: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct HistoryPage {
    pub hashes: Vec<String>,
    pub next_after: Option<String>,
}

#[derive(Clone, Debug)]
pub struct LogPage {
    pub entries: Vec<CommitInfo>,
    pub next_after: Option<String>,
}

#[derive(Clone, Debug)]
pub struct ChangedFile {
    pub path: String,
    pub status: String,
    pub additions: usize,
    pub deletions: usize,
}

#[derive(Clone, Debug)]
pub struct CommitDiffView {
    pub commit: CommitInfo,
    pub files_changed: usize,
    pub insertions: usize,
    pub deletions: usize,
    pub files: Vec<ChangedFile>,
    pub patch_lines: Vec<String>,
    pub patch_text: String,
}

impl EntryKind {
    pub fn display_name(self, name: &str) -> String {
        if self == EntryKind::Tree {
            format!("{name}/")
        } else {
            name.to_string()
        }
    }

    pub fn display_size(self, size: Option<u64>) -> String {
        if self == EntryKind::Blob {
            size.map(crate::format::human_size)
                .unwrap_or_else(|| "--".to_string())
        } else {
            "--".to_string()
        }
    }
}
