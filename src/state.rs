use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::Context;
use tokio::sync::RwLock;

use crate::config;
use crate::database;
use crate::handler::sync;

pub struct AppState {
    pub conn: database::Connection,
    pub mark: PathBuf,
    pub repo: RwLock<HashMap<String, config::Repo>>,
    // TODO caches
}

impl AppState {
    pub fn build(mut config: config::Config) -> Self {
        config.canonicalize().expect("realpath");
        config.init_marks().expect("marks");

        let mark = config.path.clone();

        if rusqlite::version_number() < 303500 {
            unimplemented!()
        };

        let conn = database::Connection::new(&config.path).expect("sqlite");
        let hash = config.into_hash();

        git2::opts::enable_caching(false);
        git2::opts::strict_hash_verification(false);

        Self {
            conn,
            mark,
            repo: RwLock::new(hash),
        }
    }

    pub fn sync_all(&mut self) {
        let conn = self.conn.blocking_lock();

        // repository: conn, Repo
        for (name, repo) in self.repo.get_mut() {
            let mut r = database::Repository::from(name);
            r.get_id_by_name(&conn)
                .with_context(|| format!("Failed to store Repository: {}", name))
                .expect("Failed to sync Repository");
            repo.repo_id = r.repo_id;
        }
        drop(conn);

        self.sync_repo();
    }

    fn sync_repo(&self) {
        for (_name, repo) in self.repo.blocking_read().iter() {
            self.sync_repo_commit(repo).unwrap();
            self.sync_repo_refs(repo, Vec::new()).unwrap();
        }
    }

    pub fn sync_repo_one(&self, repo_path: &Path, refs: Vec<String>) {
        let repo = self.repo.blocking_read();
        if let Some((_name, repo)) = repo.iter().find(|(_, v)| v.path == repo_path) {
            self.sync_repo_commit(repo).unwrap();
            self.sync_repo_refs(repo, refs).unwrap();
        }
    }

    // commit: conn, mark, repo(repo_id, repo_path)
    pub fn sync_repo_commit(&self, repo: &config::Repo) -> anyhow::Result<u32> {
        let mut count = 0;
        let mut iter = sync::CommitExportIter::new(&repo, &self.mark)
            .expect("Failed to init CommitExportIter");
        for mut c in iter.by_ref() {
            {
                let conn = self.conn.blocking_lock();
                c.insert(&conn)?;
            }
            count += 1;
            if count % 1000 == 0 {
                println!("count: {}", count);
            }
        }
        iter.close();
        Ok(count)
    }

    // refs: conn, refs, repo(repo_id, repo_path)
    pub fn sync_repo_refs(&self, repo: &config::Repo, refs: Vec<String>) -> anyhow::Result<u32> {
        let mut count = 0;
        let mut iter =
            sync::RefsExportIter::new(&repo, refs).expect("Failed to init RefsExportIter");

        let conn = self.conn.blocking_lock();
        conn.execute("BEGIN TRANSACTION", ())?;
        for mut r in iter.by_ref() {
            r.upsert(&conn)?;
            count += 1;
            if count % 100 == 0 {
                println!("count: {}", count);
            }
        }
        conn.execute("COMMIT", ())?;

        Ok(count)
    }
}
