pub mod channel;
pub mod commit;
pub mod oid;
pub mod reference;
pub mod repository;

pub use channel::Connection;
pub use commit::Commit;
pub use commit::File;
pub use oid::SqlOid;
pub use reference::Reference;
pub use repository::Repository;

impl Connection {
    pub fn init<P>(path: P) -> Result<Self, rusqlite::Error>
    where
        P: AsRef<std::path::Path>,
    {
        let db_path = path.as_ref().join(".bushi.db");
        let conn = rusqlite::Connection::open(db_path)?;

        conn.pragma_update(None, "synchronous", "OFF")?;
        conn.pragma_update(None, "journal_mode", "MEMORY")?;

        conn.execute_batch(include_str!("init.sql"))?;
        conn.set_prepared_statement_cache_capacity(64);

        Ok(Self::new(conn))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_table() {
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let db_path = temp_dir.path().to_path_buf();

        let conn = Connection::init(&db_path).expect("Failed to create database connection");

        let result = conn
            .call(|conn| conn.query_row("SELECT 1 + 2", [], |row| row.get::<_, i32>(0)))
            .await;

        assert_eq!(result, Ok(3));
    }
}
