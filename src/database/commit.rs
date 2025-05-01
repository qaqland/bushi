use std::fmt;

use indoc::indoc;
use rusqlite::{named_params, Connection, OptionalExtension};

use super::SqlOid;

pub struct Commit {
    pub commit_id: i64,
    pub commit_hash: SqlOid,
    pub commit_mark: i64,
    pub depth: i64,
    pub repo_id: i64,
    pub parent_id: i64,
    pub parent_mark: i64,
    pub files: Vec<File>,
}

impl fmt::Display for Commit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} mark: {:5}^{:<5} id: {}, depth: {}, files: {}",
            self.commit_hash.to_string(),
            self.commit_mark,
            self.parent_mark,
            self.commit_id,
            self.depth,
            self.files.len()
        )
    }
}

impl Commit {
    pub fn get_id_depth_by_mark(
        repo_id: i64,
        commit_mark: i64,
        conn: &Connection,
    ) -> Result<(i64, i64), rusqlite::Error> {
        if commit_mark == 0 {
            return Ok((0, 0));
        }
        let mut stmt = conn.prepare_cached(indoc! { r#"
        SELECT commit_id, depth FROM `commits`
        WHERE repo_id = :repo_id AND commit_mark = :commit_mark
        LIMIT 1;
        "# })?;
        let (commit_id, depth) = stmt.query_row(
            named_params! {":repo_id": repo_id, ":commit_mark": commit_mark},
            |row| {
                let commit_id = row.get::<_, i64>(0)?;
                let depth = row.get::<_, i64>(1).unwrap_or(0);
                Ok((commit_id, depth))
            },
        )?;
        Ok((commit_id, depth))
    }

    pub fn get_id_by_hash(
        repo_id: i64,
        commit_hash: &SqlOid,
        conn: &Connection,
    ) -> Result<i64, rusqlite::Error> {
        if commit_hash.is_zero() {
            return Ok(0);
        }
        let mut stmt = conn.prepare_cached(indoc! { r#"
        SELECT commit_id FROM `commits`
        WHERE repo_id = :repo_id AND commit_hash = :commit_hash
        LIMIT 1;
        "# })?;
        let commit_id = stmt.query_row(
            named_params! {":repo_id": repo_id, ":commit_hash": commit_hash},
            |row| row.get::<_, i64>(0),
        )?;
        Ok(commit_id)
    }

    pub fn insert(&mut self, conn: &Connection) -> Result<(), rusqlite::Error> {
        if self.repo_id == 0 {
            todo!()
        }

        conn.execute("BEGIN TRANSACTION", ())?;

        let mut stmt1 = conn.prepare_cached(indoc! { r#"
        INSERT INTO commits(commit_hash, commit_mark, depth, repo_id, parent_id)
        VALUES (:commit_hash, :commit_mark, :depth, :repo_id, :parent_id);
        "#})?;

        // maybe zero
        let (parent_id, depth) = Self::get_id_depth_by_mark(self.repo_id, self.parent_mark, conn)?;

        if parent_id == 0 {
            println!("ROOT {}", self);
            stmt1.execute(named_params! {
                ":commit_hash": self.commit_hash,
                ":commit_mark": self.commit_mark,
                ":depth": 0, // root
                ":repo_id": self.repo_id,
                ":parent_id": rusqlite::types::Null,
            })?;
        } else {
            stmt1.execute(named_params! {
                ":commit_hash": self.commit_hash,
                ":commit_mark": self.commit_mark,
                ":depth": depth + 1,
                ":repo_id": self.repo_id,
                ":parent_id": parent_id,
            })?;
        }
        let (commit_id, _) = Self::get_id_depth_by_mark(self.repo_id, self.commit_mark, conn)?;

        let mut stmt2 = conn.prepare_cached(indoc! {r#"
        INSERT OR IGNORE INTO commit_files(commit_id, file_id)
        VALUES (:commit_id, :file_id);
        "#})?;

        for f in &mut self.files {
            let file_id = f.get_id_by_name(conn)?;
            stmt2.execute(named_params! {
                ":commit_id": commit_id,
                ":file_id": file_id,
            })?;
        }

        // println!("files done");

        // println!("ancestor start");
        let mut stmt3 = conn.prepare_cached(include_str!("insert-ancestor.sql"))?;
        stmt3.execute(named_params! {
            ":commit_id": commit_id,
        })?;
        // println!("ancestor done");

        conn.execute("COMMIT", ())?;
        Ok(())
    }
}

pub struct File {
    pub file_id: i64,
    pub name: String,
}

impl File {
    /// select or insert
    pub fn get_id_by_name(&mut self, conn: &Connection) -> Result<i64, rusqlite::Error> {
        if self.file_id != 0 {
            return Ok(self.file_id);
        }

        let mut stmt1 = conn.prepare_cached(indoc! { r#"
        SELECT file_id FROM files WHERE name = :name LIMIT 1
        "#})?;

        // select first
        let file_id_or_none = stmt1
            .query_row(named_params! {":name": self.name}, |row| {
                row.get::<_, i64>(0)
            })
            .optional()?;

        if let Some(file_id) = file_id_or_none {
            self.file_id = file_id;
        } else {
            // then try insert
            let mut stmt2 = conn.prepare_cached(indoc! { r#"
            INSERT OR IGNORE INTO files(name) VALUES (:name)
            "#})?;
            self.file_id = stmt2.insert(named_params! {":name": self.name})?;
        }

        Ok(self.file_id)
    }
}

impl From<String> for File {
    fn from(name: String) -> Self {
        File { file_id: 0, name }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_hashmap() {
        let conn = Connection::open_in_memory().expect("open sqlite in memory");
        conn.execute_batch(include_str!("init.sql")).expect("init");

        let mut file = File {
            file_id: 0,
            name: "README.md".to_string(),
        };

        let id_1 = file.get_id_by_name(&conn).expect("insert *1");
        assert_eq!(id_1, 1);
        let id_2 = file.get_id_by_name(&conn).expect("insert *2");
        assert_eq!(id_2, 1);
    }
}
