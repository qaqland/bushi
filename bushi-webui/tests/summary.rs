mod helpers;

use bushi_webui::error::DomainError;
use bushi_webui::page::summary;
use helpers::{git_port, repo_port};

#[tokio::test]
async fn summary_has_repo_name() {
    let repo = repo_port();
    let git = git_port();
    let view = summary::load(&repo, &git, "test-repo").await.unwrap();
    assert_eq!(view.repo.name, "test-repo");
}

#[tokio::test]
async fn summary_has_recent_commits_in_descending_order() {
    let repo = repo_port();
    let git = git_port();
    let view = summary::load(&repo, &git, "test-repo").await.unwrap();
    assert!(!view.recent.is_empty());
    assert_eq!(view.recent[0].subject, "m-0000020");
    assert_eq!(view.recent[1].subject, "m-0000019");
}

#[tokio::test]
async fn summary_recent_commit_has_author() {
    let repo = repo_port();
    let git = git_port();
    let view = summary::load(&repo, &git, "test-repo").await.unwrap();
    assert_eq!(view.recent[0].author, "Test");
    assert_eq!(view.recent[0].author_email, "test@qaq.land");
}

#[tokio::test]
async fn summary_repo_not_found() {
    let repo = repo_port();
    let git = git_port();
    let err = summary::load(&repo, &git, "no-such-repo")
        .await
        .unwrap_err();
    assert!(matches!(err, DomainError::RepoNotFound));
}
