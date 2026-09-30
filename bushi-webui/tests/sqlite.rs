mod helpers;

use bushi_webui::data::{RepositoryPort, SqliteRepository};
use helpers::repo_port;

#[tokio::test]
async fn get_repository_found() {
    let repo = repo_port();
    let rec = repo.get_repository("test-repo").await.unwrap().unwrap();
    assert_eq!(rec.name, "test-repo");
}

#[tokio::test]
async fn get_repository_not_found() {
    let repo = repo_port();
    let result = repo.get_repository("no-such-repo").await.unwrap();
    assert!(result.is_none());
}

#[tokio::test]
async fn log_paginates_correctly() {
    let repo = repo_port();
    let rec = repo.get_repository("test-repo").await.unwrap().unwrap();
    let rev = repo.resolve_rev(rec.id, "main").await.unwrap();

    let page1 = repo.log(rec.id, &rev.commit_hash, None, 5).await.unwrap();
    assert_eq!(page1.hashes.len(), 5);
    assert!(page1.next_after.is_some());

    let page2 = repo
        .log(rec.id, &rev.commit_hash, page1.next_after.as_deref(), 5)
        .await
        .unwrap();
    assert_eq!(page2.hashes.len(), 5);
    assert!(page1.hashes.iter().all(|h| !page2.hashes.contains(h)));
}

#[tokio::test]
async fn path_history_returns_my_txt_commits() {
    let repo = repo_port();
    let rec = repo.get_repository("test-repo").await.unwrap().unwrap();
    let rev = repo.resolve_rev(rec.id, "main").await.unwrap();

    let page = repo
        .path_history(rec.id, "my.txt", &rev.commit_hash, None, 30)
        .await
        .unwrap();
    assert_eq!(page.hashes.len(), 2);
}

#[tokio::test]
async fn ref_time_returns_some_for_existing_repo() {
    let repo = repo_port();
    let rec = repo.get_repository("test-repo").await.unwrap().unwrap();
    let time = repo.ref_time(rec.id, "main").await.unwrap();
    assert!(time.is_some());
}

#[tokio::test]
async fn missing_database_is_an_error() {
    let dir = tempfile::tempdir().unwrap();
    let missing = dir.path().join("missing.db");
    assert!(SqliteRepository::new(missing.to_str().unwrap()).is_err());
}
