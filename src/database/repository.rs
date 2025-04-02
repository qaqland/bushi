use indoc::indoc;
use rusqlite::{named_params, Connection, OptionalExtension};

pub struct Repository {
    pub repo_id: i64,
    pub name: String,
}

impl From<&String> for Repository {
    fn from(name: &String) -> Self {
        Repository {
            repo_id: 0,
            name: name.clone(),
        }
    }
}

impl Repository {
    /// Same as Files, select or insert
    pub fn get_id_by_name(&mut self, conn: &Connection) -> Result<i64, rusqlite::Error> {
        if self.repo_id != 0 {
            return Ok(self.repo_id);
        }

        let mut stmt1 = conn.prepare_cached(indoc! { r#"
        SELECT repo_id FROM repositories WHERE name = :name LIMIT 1
        "#})?;

        let repo_id_or_none = stmt1
            .query_row(named_params! {":name": self.name}, |row| {
                row.get::<_, i64>(0)
            })
            .optional()?;

        if let Some(repo_id) = repo_id_or_none {
            self.repo_id = repo_id;
        } else {
            let mut stmt2 = conn.prepare_cached(indoc! { r#"
            INSERT OR IGNORE INTO repositories(name) VALUES (:name)
            "#})?;
            self.repo_id = stmt2.insert(named_params! {":name": self.name})?;
        }

        Ok(self.repo_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_repository() {
        let conn = Connection::open_in_memory().expect("open sqlite in memory");
        conn.execute_batch(include_str!("init.sql")).expect("init");

        let mut repo = Repository {
            repo_id: 0,
            name: "aports".to_string(),
        };

        let id_1 = repo.get_id_by_name(&conn).expect("insert *1");
        let id_2 = repo.get_id_by_name(&conn).expect("insert *2");
        assert_eq!(id_1, 1);
        assert_eq!(id_2, 1);

        repo = Repository {
            repo_id: 0,
            name: "gcc".to_string(),
        };
        let id_3 = repo.get_id_by_name(&conn).expect("insert *3");
        let id_4 = repo.get_id_by_name(&conn).expect("insert *4");
        assert_eq!(id_3, id_4);
    }
}
