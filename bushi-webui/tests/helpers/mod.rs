use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

use bushi_webui::data::{GitRepository, SqliteRepository};

static HISTORY_REPO: OnceLock<PathBuf> = OnceLock::new();

pub fn history_repo() -> &'static Path {
    HISTORY_REPO.get_or_init(|| {
        let script = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../bushi-utils/history/make-repo.sh"
        );
        let out = Command::new("sh")
            .env("TOTAL", "20")
            .arg(script)
            .output()
            .expect("run make-repo.sh");
        assert!(
            out.status.success(),
            "make-repo.sh failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../bushi-utils/history/test-repo"
        ))
    })
}

static DB_PATH: OnceLock<String> = OnceLock::new();

pub fn db_path() -> &'static str {
    DB_PATH.get_or_init(|| {
        let repo = history_repo();
        let bin = index_bin();
        let db = repo.parent().unwrap().join("test.db");
        let db_str = db.to_str().unwrap();

        let out = Command::new(&bin)
            .args(["-t", db_str, "-l"])
            .output()
            .expect("bushi-index -l");
        assert!(
            out.status.success(),
            "bushi-index -l failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );

        let out = Command::new(&bin)
            .args(["-t", db_str, "-a"])
            .arg(repo)
            .output()
            .expect("bushi-index -a");
        assert!(
            out.status.success(),
            "bushi-index -a failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );

        let out = Command::new(&bin)
            .args(["-t", db_str, "test-repo"])
            .output()
            .expect("bushi-index sync");
        assert!(
            out.status.success(),
            "bushi-index sync failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );

        db_str.to_string()
    })
}

pub fn repo_port() -> SqliteRepository {
    SqliteRepository::new(db_path()).unwrap()
}

#[allow(dead_code)]
pub fn git_port() -> GitRepository {
    GitRepository::new()
}

fn index_bin() -> String {
    std::env::var("BUSHI_INDEX_BIN")
        .unwrap_or_else(|_| "../bushi-index/build/bushi-index".to_string())
}
