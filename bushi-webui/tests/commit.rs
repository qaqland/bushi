mod helpers;

use bushi_webui::error::DomainError;
use bushi_webui::page::commit;
use helpers::{git_port, repo_port};

#[tokio::test]
async fn commit_has_info() {
    let repo = repo_port();
    let git = git_port();
    let page = commit::load(&repo, &git, "test-repo", "main")
        .await
        .unwrap();
    let diff = page.diff;
    assert_eq!(diff.commit.subject, "m-0000020");
    assert_eq!(diff.commit.author, "Test");
}

#[tokio::test]
async fn commit_has_diff() {
    let repo = repo_port();
    let git = git_port();
    let page = commit::load(&repo, &git, "test-repo", "main")
        .await
        .unwrap();
    let diff = page.diff;
    assert!(!diff.files.is_empty());
    assert!(!diff.patch_text.is_empty());
}

#[tokio::test]
async fn commit_rev_not_found() {
    let repo = repo_port();
    let git = git_port();
    let err = commit::load(
        &repo,
        &git,
        "test-repo",
        "0000000000000000000000000000000000000000",
    )
    .await
    .unwrap_err();
    assert!(matches!(err, DomainError::RevNotFound));
}
