mod helpers;

use bushi_webui::data::EntryKind;
use bushi_webui::error::DomainError;
use bushi_webui::page::tree;
use helpers::{git_port, repo_port};

#[tokio::test]
async fn tree_root_lists_entries() {
    let repo = repo_port();
    let git = git_port();
    let view = tree::load(&repo, &git, "test-repo", "main", String::new())
        .await
        .unwrap();
    assert!(!view.entries.is_empty());
}

#[tokio::test]
async fn tree_root_has_my_txt() {
    let repo = repo_port();
    let git = git_port();
    let view = tree::load(&repo, &git, "test-repo", "main", String::new())
        .await
        .unwrap();
    let names: Vec<&str> = view.entries.iter().map(|e| e.name.as_str()).collect();
    assert!(names.contains(&"my.txt"));
}

#[tokio::test]
async fn tree_dirs_before_files() {
    let repo = repo_port();
    let git = git_port();
    let view = tree::load(&repo, &git, "test-repo", "main", String::new())
        .await
        .unwrap();
    let mut last_was_tree = true;
    for entry in &view.entries {
        if !last_was_tree && entry.kind == EntryKind::Tree {
            panic!("tree entry after blob entry");
        }
        last_was_tree = entry.kind == EntryKind::Tree;
    }
}

#[tokio::test]
async fn tree_path_not_found() {
    let repo = repo_port();
    let git = git_port();
    let err = tree::load(&repo, &git, "test-repo", "main", "no/such/path".to_string())
        .await
        .unwrap_err();
    assert!(matches!(err, DomainError::PathNotFound));
}
