use tokio::sync::Mutex;

pub mod commit;
pub mod repository;
pub mod reference;

pub struct Connection(Mutex<rusqlite::Connection>);

pub use commit::Commit as Commit;
pub use commit::File as File;
pub use repository::Repository as Repository;

impl Connection {
    pub fn new<P>(path: P) -> Result<Self, rusqlite::Error>
    where
        P: AsRef<std::path::Path>,
    {
        let db_path = path.as_ref().join(".bushi.db");
        let conn = rusqlite::Connection::open(db_path)?;

        conn.pragma_update(None, "synchronous", "OFF")?;
        conn.pragma_update(None, "journal_mode", "MEMORY")?;

        conn.execute_batch(include_str!("init.sql"))?;
        conn.set_prepared_statement_cache_capacity(64);
        Ok(Self(Mutex::new(conn)))
    }
}

impl std::ops::Deref for Connection {
    type Target = Mutex<rusqlite::Connection>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for Connection {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_connection_select_async() {
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let db_path = temp_dir.path().to_path_buf();
        let connection = Connection::new(&db_path).expect("Failed to create database connection");

        let result = {
            let conn = connection.lock().await;
            conn.query_row("SELECT 1 + 1", [], |row| row.get::<_, i32>(0))
                .expect("Failed to query database")
        };

        assert_eq!(result, 2);
    }

    #[test]
    fn test_connection_select_blocking() {
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let db_path = temp_dir.path().to_path_buf();
        let connection = Connection::new(&db_path).expect("Failed to create database connection");

        let result = {
            let conn = connection.blocking_lock();
            conn.query_row("SELECT 1 + 2", [], |row| row.get::<_, i32>(0))
                .expect("Failed to query database")
        };

        assert_eq!(result, 3);
    }
}
