mod helpers;

use bushi_webui::error::DomainError;
use bushi_webui::page::blob;
use helpers::{git_port, repo_port};

#[tokio::test]
async fn blob_returns_file_content() {
    let repo = repo_port();
    let git = git_port();
    let view = blob::load(&repo, &git, "test-repo", "main", "my.txt".into())
        .await
        .unwrap();
    assert_eq!(view.path, "my.txt");
    let text = view.blob.text_lines.unwrap().join("\n");
    assert!(text.contains("c-0000020"));
}

#[tokio::test]
async fn blob_has_size() {
    let repo = repo_port();
    let git = git_port();
    let view = blob::load(&repo, &git, "test-repo", "main", "my.txt".into())
        .await
        .unwrap();
    assert!(view.blob.size > 0);
}

#[tokio::test]
async fn blob_path_not_found() {
    let repo = repo_port();
    let git = git_port();
    let err = blob::load(&repo, &git, "test-repo", "main", "nonexistent.txt".into())
        .await
        .unwrap_err();
    assert!(matches!(err, DomainError::PathNotFound));
}
