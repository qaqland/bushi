use async_trait::async_trait;
use deadpool_sqlite::{Config, Pool, Runtime, rusqlite};
use moka::future::Cache;
use rusqlite::{OptionalExtension, params};

use crate::data::{
    HistoryPage, RefRecord, RepositoryPort, RepositoryRecord, ResolvedRev, short_hash,
};
use crate::error::DomainError;

const STATEMENT_CACHE_CAPACITY: usize = 64;

#[derive(Clone)]
pub struct SqliteRepository {
    pool: Pool,
    repo_cache: Cache<String, RepositoryRecord>,
}

impl SqliteRepository {
    pub fn new(path: impl Into<String>) -> anyhow::Result<Self> {
        let path = path.into();
        anyhow::ensure!(
            std::path::Path::new(&path).exists(),
            "database not found: {path}"
        );
        let cfg = Config::new(path);
        let pool = cfg.create_pool(Runtime::Tokio1)?;
        Ok(Self {
            pool,
            repo_cache: Cache::new(256),
        })
    }

    async fn interact<T, F>(&self, f: F) -> Result<T, DomainError>
    where
        T: Send + 'static,
        F: FnOnce(&rusqlite::Connection) -> rusqlite::Result<T> + Send + 'static,
    {
        self.interact_domain(move |conn| f(conn).map_err(DomainError::from))
            .await
    }

    async fn interact_domain<T, F>(&self, f: F) -> Result<T, DomainError>
    where
        T: Send + 'static,
        F: FnOnce(&rusqlite::Connection) -> Result<T, DomainError> + Send + 'static,
    {
        let conn = self.pool.get().await?;
        conn.interact(move |conn| {
            conn.set_prepared_statement_cache_capacity(STATEMENT_CACHE_CAPACITY);
            conn.execute_batch(
                "PRAGMA query_only = ON;
                 PRAGMA foreign_keys = ON;
                 PRAGMA busy_timeout = 5000;",
            )?;
            f(conn)
        })
        .await?
    }
}

#[async_trait]
impl RepositoryPort for SqliteRepository {
    async fn list_repositories(&self) -> Result<Vec<RepositoryRecord>, DomainError> {
        self.interact(|conn| {
            let mut stmt = conn.prepare_cached(
                "SELECT repository_id
                      , repository_name
                      , repository_path
                      , repository_head
                   FROM repositories
                  ORDER BY repository_name;",
            )?;
            let rows = stmt.query_map([], read_repository)?;
            rows.collect()
        })
        .await
    }

    async fn get_repository(&self, name: &str) -> Result<Option<RepositoryRecord>, DomainError> {
        if let Some(repo) = self.repo_cache.get(name).await {
            return Ok(Some(repo));
        }
        let name = name.to_string();
        let repo = self
            .interact(move |conn| {
                let mut stmt = conn.prepare_cached(
                    "SELECT repository_id
                          , repository_name
                          , repository_path
                          , repository_head
                       FROM repositories
                      WHERE repository_name = ?1
                      LIMIT 1;",
                )?;
                stmt.query_row(params![name], read_repository).optional()
            })
            .await?;
        if let Some(ref repo) = repo {
            self.repo_cache
                .insert(repo.name.clone(), repo.clone())
                .await;
        }
        Ok(repo)
    }

    async fn default_rev(&self, repo: &RepositoryRecord) -> Result<String, DomainError> {
        if let Some(head) = normalize_head(repo.head.as_deref()) {
            return Ok(head);
        }
        let repo_id = repo.id;
        self.interact(move |conn| {
            let mut stmt = conn.prepare_cached(
                "SELECT show_name
                   FROM refs
                  WHERE repository_id = ?1
                    AND ref_type = 0
                    AND show_name = 'master'
                  LIMIT 1;",
            )?;
            let master: Option<String> = stmt
                .query_row(params![repo_id], |row| row.get(0))
                .optional()?;
            if let Some(master) = master {
                return Ok(master);
            }
            let mut stmt = conn.prepare_cached(
                "SELECT show_name
                   FROM refs
                  WHERE repository_id = ?1
                    AND ref_type = 0
                  ORDER BY show_name
                  LIMIT 1;",
            )?;
            let branch: Option<String> = stmt
                .query_row(params![repo_id], |row| row.get(0))
                .optional()?;
            if let Some(branch) = branch {
                return Ok(branch);
            }
            let mut stmt = conn.prepare_cached(
                "SELECT show_name
                   FROM refs
                  WHERE repository_id = ?1
                    AND ref_type = 1
                  ORDER BY show_name
                  LIMIT 1;",
            )?;
            let tag: Option<String> = stmt
                .query_row(params![repo_id], |row| row.get(0))
                .optional()?;
            let rev = tag
                .map(|name| format!("tag/{name}"))
                .unwrap_or_else(|| "master".to_string());
            Ok(rev)
        })
        .await
    }

    async fn resolve_rev(&self, repo_id: i64, rev: &str) -> Result<ResolvedRev, DomainError> {
        if let Some(tag) = rev.strip_prefix("tag/") {
            return self.resolve_ref(repo_id, rev, 1, tag).await;
        }
        if rev.len() >= 8 && rev.bytes().all(|b| b.is_ascii_hexdigit()) {
            return self.resolve_hash(repo_id, rev).await;
        }
        self.resolve_ref(repo_id, rev, 0, rev).await
    }

    async fn ref_time(&self, repo_id: i64, rev: &str) -> Result<Option<i64>, DomainError> {
        let (ref_type, show_name) = match rev.strip_prefix("tag/") {
            Some(tag) => (1, tag.to_string()),
            None => (0, rev.to_string()),
        };
        self.interact(move |conn| {
            let mut stmt = conn.prepare_cached(
                "SELECT ref_time
                   FROM refs
                  WHERE repository_id = ?1
                    AND ref_type = ?2
                    AND show_name = ?3
                  LIMIT 1;",
            )?;
            let exact: Option<i64> = stmt
                .query_row(params![repo_id, ref_type, show_name], |row| row.get(0))
                .optional()?;
            if exact.is_some() {
                return Ok(exact);
            }
            let mut stmt = conn.prepare_cached(
                "SELECT MAX(ref_time)
                       FROM refs
                      WHERE repository_id = ?1;",
            )?;
            stmt.query_row(params![repo_id], |row| row.get(0))
        })
        .await
    }

    async fn list_refs(&self, repo_id: i64, ref_type: i64) -> Result<Vec<RefRecord>, DomainError> {
        self.interact(move |conn| {
            let mut stmt = conn.prepare_cached(
                "SELECT r.show_name
                      , c.commit_hash
                   FROM refs AS r
                   JOIN commits AS c
                     ON c.commit_id = r.commit_id
                  WHERE r.repository_id = ?1
                    AND r.ref_type = ?2
                  ORDER BY r.show_name;",
            )?;
            let rows = stmt.query_map(params![repo_id, ref_type], |row| {
                Ok(RefRecord {
                    show_name: row.get(0)?,
                    commit_hash: row.get(1)?,
                })
            })?;
            rows.collect()
        })
        .await
    }

    async fn log(
        &self,
        repo_id: i64,
        start_commit_hash: &str,
        after: Option<&str>,
        limit: usize,
    ) -> Result<HistoryPage, DomainError> {
        let start_commit_hash = start_commit_hash.to_string();
        let after = after.map(|s| s.to_string());
        self.interact_domain(move |conn| {
            let start_commit_id = match after.as_deref() {
                Some(cursor) => {
                    let cursor_id = commit_id_by_prefix(conn, repo_id, cursor)?;
                    first_parent(conn, cursor_id)?
                }
                None => Some(commit_id_by_prefix(conn, repo_id, &start_commit_hash)?),
            };
            let Some(start_commit_id) = start_commit_id else {
                return Ok(HistoryPage {
                    hashes: Vec::new(),
                    next_after: None,
                });
            };

            let mut stmt = conn.prepare_cached(
                "WITH RECURSIVE history(commit_id, seq) AS (
                     SELECT ?1
                          , 0
                     UNION ALL
                     SELECT a.ancestor_id
                          , h.seq + 1
                       FROM history AS h
                       JOIN ancestors AS a
                         ON a.commit_id = h.commit_id
                        AND a.exponent = 0
                      ORDER BY 2
                      LIMIT ?2
                  )
                  SELECT c.commit_hash
                    FROM history AS h
                    JOIN commits AS c
                      ON c.commit_id = h.commit_id;",
            )?;
            let mut rows = stmt.query(params![start_commit_id, (limit + 1) as i64])?;
            let mut hashes = Vec::new();
            while let Some(row) = rows.next()? {
                hashes.push(row.get(0)?);
            }
            Ok(page_from_hashes(hashes, limit))
        })
        .await
    }

    async fn path_history(
        &self,
        repo_id: i64,
        path: &str,
        start_commit_hash: &str,
        after: Option<&str>,
        limit: usize,
    ) -> Result<HistoryPage, DomainError> {
        let path = path.to_string();
        let start_commit_hash = start_commit_hash.to_string();
        let after = after.map(|s| s.to_string());
        self.interact_domain(move |conn| {
            let path_id = lookup_path_id(conn, &path)?;
            let Some(path_id) = path_id else {
                return Ok(HistoryPage {
                    hashes: Vec::new(),
                    next_after: None,
                });
            };

            let start_commit_id = match after.as_deref() {
                Some(cursor) => {
                    let cursor_id = commit_id_by_prefix(conn, repo_id, cursor)?;
                    next_on_path_chain(conn, cursor_id, path_id)?
                }
                None => {
                    let input_id = commit_id_by_prefix(conn, repo_id, &start_commit_hash)?;
                    find_path_start_commit(conn, repo_id, path_id, input_id)?
                }
            };
            let Some(start_commit_id) = start_commit_id else {
                return Ok(HistoryPage {
                    hashes: Vec::new(),
                    next_after: None,
                });
            };

            let mut stmt = conn.prepare_cached(
                "WITH RECURSIVE history(commit_id, seq) AS (
                     SELECT ?1
                          , 0
                     UNION ALL
                     SELECT cg.last_commit_id
                          , h.seq + 1
                       FROM history AS h
                       JOIN changes AS cg
                         ON cg.commit_id = h.commit_id
                        AND cg.path_id = ?2
                      WHERE cg.last_commit_id != h.commit_id
                      ORDER BY 2
                      LIMIT ?3
                  )
                  SELECT c.commit_hash
                    FROM history AS h
                    JOIN commits AS c
                      ON c.commit_id = h.commit_id;",
            )?;
            let mut rows = stmt.query(params![start_commit_id, path_id, (limit + 1) as i64])?;
            let mut hashes = Vec::new();
            while let Some(row) = rows.next()? {
                hashes.push(row.get(0)?);
            }
            Ok(page_from_hashes(hashes, limit))
        })
        .await
    }

    async fn latest_change(
        &self,
        repo_id: i64,
        start_commit_hash: &str,
        path: &str,
    ) -> Result<Option<String>, DomainError> {
        let page = self
            .path_history(repo_id, path, start_commit_hash, None, 1)
            .await?;
        let latest = page.hashes.into_iter().next();
        Ok(latest)
    }

    async fn path_kind(&self, path: &str) -> Result<Option<bool>, DomainError> {
        let path = path.to_string();
        self.interact_domain(move |conn| {
            if path_id_by_name(conn, &path)?.is_some() {
                return Ok(Some(true));
            }
            if !path.is_empty()
                && !path.ends_with('/')
                && path_id_by_name(conn, &format!("{path}/"))?.is_some()
            {
                return Ok(Some(false));
            }
            Ok(None)
        })
        .await
    }
}

fn lookup_path_id(conn: &rusqlite::Connection, path: &str) -> Result<Option<i64>, DomainError> {
    if let Some(id) = path_id_by_name(conn, path)? {
        return Ok(Some(id));
    }
    if !path.is_empty() && !path.ends_with('/') {
        return path_id_by_name(conn, &format!("{path}/"));
    }
    Ok(None)
}

fn path_id_by_name(conn: &rusqlite::Connection, name: &str) -> Result<Option<i64>, DomainError> {
    let mut stmt = conn.prepare_cached(
        "SELECT path_id
           FROM paths
          WHERE name = ?1
          LIMIT 1;",
    )?;
    let path_id = stmt.query_row(params![name], |row| row.get(0)).optional()?;
    Ok(path_id)
}

fn commit_id_by_prefix(
    conn: &rusqlite::Connection,
    repo_id: i64,
    prefix: &str,
) -> Result<i64, DomainError> {
    let like = format!("{prefix}%");
    let mut stmt = conn.prepare_cached(
        "SELECT commit_id
           FROM commits
          WHERE repository_id = ?1
            AND commit_hash LIKE ?2
          LIMIT 2;",
    )?;
    let rows = stmt
        .query_map(params![repo_id, like], |row| row.get::<_, i64>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    match rows.as_slice() {
        [commit_id] => Ok(*commit_id),
        [] => Err(DomainError::RevNotFound),
        _ => Err(DomainError::AmbiguousHash),
    }
}

fn first_parent(conn: &rusqlite::Connection, commit_id: i64) -> Result<Option<i64>, DomainError> {
    let mut stmt = conn.prepare_cached(
        "SELECT ancestor_id
           FROM ancestors
          WHERE commit_id = ?1
            AND exponent = 0;",
    )?;
    let parent = stmt
        .query_row(params![commit_id], |row| row.get(0))
        .optional()?;
    Ok(parent)
}

fn next_on_path_chain(
    conn: &rusqlite::Connection,
    cursor_id: i64,
    path_id: i64,
) -> Result<Option<i64>, DomainError> {
    let mut stmt = conn.prepare_cached(
        "SELECT last_commit_id
           FROM changes
          WHERE commit_id = ?1
            AND path_id = ?2;",
    )?;
    let last: Option<i64> = stmt
        .query_row(params![cursor_id, path_id], |row| row.get(0))
        .optional()?
        .flatten();
    match last {
        Some(last_id) if last_id != cursor_id => Ok(Some(last_id)),
        _ => Ok(None),
    }
}

fn find_path_start_commit(
    conn: &rusqlite::Connection,
    repo_id: i64,
    path_id: i64,
    input_commit_id: i64,
) -> Result<Option<i64>, DomainError> {
    let mut stmt = conn.prepare_cached(
        "SELECT first_depth
           FROM commits
          WHERE commit_id = ?1;",
    )?;
    let input_depth: i64 = stmt.query_row(params![input_commit_id], |row| row.get(0))?;
    let mut stmt = conn.prepare_cached(
        "SELECT cg.commit_id
           FROM changes AS cg
           JOIN commits AS c
             ON c.commit_id = cg.commit_id
          WHERE cg.path_id = ?1
            AND c.repository_id = ?2
            AND c.first_depth <= ?3
          ORDER BY c.first_depth DESC;",
    )?;
    let mut rows = stmt.query(params![path_id, repo_id, input_depth])?;
    while let Some(row) = rows.next()? {
        let candidate_id: i64 = row.get(0)?;
        if is_first_parent_ancestor(conn, candidate_id, input_commit_id)? {
            return Ok(Some(candidate_id));
        }
    }
    Ok(None)
}

fn page_from_hashes(mut hashes: Vec<String>, limit: usize) -> HistoryPage {
    let next_after = if hashes.len() > limit {
        Some(short_hash(&hashes[limit - 1]))
    } else {
        None
    };
    hashes.truncate(limit);
    HistoryPage { hashes, next_after }
}

impl SqliteRepository {
    async fn resolve_ref(
        &self,
        repo_id: i64,
        input: &str,
        ref_type: i64,
        show_name: &str,
    ) -> Result<ResolvedRev, DomainError> {
        let input = input.to_string();
        let show_name = show_name.to_string();
        self.interact(move |conn| {
            let mut stmt = conn.prepare_cached(
                "SELECT c.commit_hash
                   FROM refs AS r
                   JOIN commits AS c
                     ON c.commit_id = r.commit_id
                  WHERE r.repository_id = ?1
                    AND r.ref_type = ?2
                    AND r.show_name = ?3
                  LIMIT 1;",
            )?;
            stmt.query_row(params![repo_id, ref_type, show_name], |row| {
                Ok(ResolvedRev {
                    input,
                    commit_hash: row.get(0)?,
                })
            })
            .optional()
        })
        .await?
        .ok_or(DomainError::RevNotFound)
    }

    async fn resolve_hash(&self, repo_id: i64, prefix: &str) -> Result<ResolvedRev, DomainError> {
        let input = prefix.to_string();
        let like = format!("{prefix}%");
        let rows = self
            .interact(move |conn| {
                let mut stmt = conn.prepare_cached(
                    "SELECT commit_hash
                       FROM commits
                      WHERE repository_id = ?1
                        AND commit_hash LIKE ?2
                      LIMIT 2;",
                )?;
                let rows = stmt.query_map(params![repo_id, like], |row| row.get::<_, String>(0))?;
                rows.collect::<rusqlite::Result<Vec<_>>>()
            })
            .await?;
        match rows.as_slice() {
            [commit_hash] => Ok(ResolvedRev {
                input,
                commit_hash: commit_hash.clone(),
            }),
            [] => Err(DomainError::RevNotFound),
            _ => Err(DomainError::AmbiguousHash),
        }
    }
}

fn is_first_parent_ancestor(
    conn: &rusqlite::Connection,
    candidate_id: i64,
    input_id: i64,
) -> rusqlite::Result<bool> {
    if candidate_id == input_id {
        return Ok(true);
    }
    let mut stmt = conn.prepare_cached(
        "SELECT first_depth
           FROM commits
          WHERE commit_id = ?1;",
    )?;
    let da: i64 = stmt.query_row(params![candidate_id], |row| row.get(0))?;
    let db: i64 = stmt.query_row(params![input_id], |row| row.get(0))?;
    if da > db {
        return Ok(false);
    }
    let mut current = input_id;
    let mut delta = (db - da) as usize;
    let mut exponent = 0usize;
    let mut stmt = conn.prepare_cached(
        "SELECT ancestor_id
           FROM ancestors
          WHERE commit_id = ?1
            AND exponent = ?2;",
    )?;
    while delta > 0 {
        if delta & 1 == 1 {
            current = stmt.query_row(params![current, exponent], |row| row.get(0))?;
        }
        delta >>= 1;
        exponent += 1;
    }
    Ok(current == candidate_id)
}

fn read_repository(row: &rusqlite::Row<'_>) -> rusqlite::Result<RepositoryRecord> {
    let head: Option<String> = row.get(3)?;
    Ok(RepositoryRecord {
        id: row.get(0)?,
        name: row.get(1)?,
        path: row.get(2)?,
        head: head.and_then(|value| {
            let value = value.trim().to_string();
            (!value.is_empty()).then_some(value)
        }),
    })
}

fn normalize_head(head: Option<&str>) -> Option<String> {
    let head = head?.trim();
    if head.is_empty() {
        return None;
    }
    let head = head.strip_prefix("refs/heads/").unwrap_or(head);
    Some(head.replace('/', ":"))
}
