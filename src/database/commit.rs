use std::fmt;

use indoc::indoc;
use rusqlite::{named_params, Connection, OptionalExtension};

use super::SqlOid;

pub struct Commit {
    pub commit_id: Option<i64>,
    pub commit_hash: SqlOid,
    pub commit_mark: i64,
    pub parent_mark: Option<i64>,
    pub repo_id: i64,
    pub depth: Option<i64>,
    pub files: Vec<String>,
}

impl fmt::Display for Commit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} id: {}, mark: {:>7}, from: {}",
            self.commit_hash.to_string(),
            self.commit_id.unwrap_or_default(),
            self.commit_mark,
            self.parent_mark.unwrap_or(-1),
        )
    }
}

impl Commit {
    pub fn find_by_id(commit_id: i64, conn: &Connection) -> Result<Commit, rusqlite::Error> {
        if commit_id == 0 {
            return Err(rusqlite::Error::QueryReturnedNoRows);
        }
        let mut stmt = conn.prepare_cached(indoc! { r#"
        SELECT
            commit_id, commit_hash, commit_mark, parent_mark, depth, repo_id
        FROM
            commits
        WHERE
            commit_id = :commit_id
        LIMIT 1
        "# })?;
        let commit = stmt.query_row(
            named_params! {
                ":commit_id": commit_id,
            },
            |row| {
                Ok(Commit {
                    commit_id: row.get(0)?,
                    commit_hash: row.get(1)?,
                    commit_mark: row.get(2)?,
                    parent_mark: row.get(3)?,
                    depth: row.get(4)?,
                    repo_id: row.get(5)?,
                    files: Vec::new(),
                })
            },
        )?;
        Ok(commit)
    }

    pub fn fetch_id_by_hash(
        repo_id: i64,
        commit_hash: &SqlOid,
        conn: &Connection,
    ) -> Result<i64, rusqlite::Error> {
        if commit_hash.is_zero() {
            return Ok(0);
        }
        let mut stmt = conn.prepare_cached(indoc! { r#"
        SELECT
            commit_id
        FROM
            commits
        WHERE
            repo_id = :repo_id
            AND commit_hash = :commit_hash
        LIMIT 1
        "# })?;
        let commit_id = stmt.query_row(
            named_params! {
                ":repo_id": repo_id,
                ":commit_hash": commit_hash,
            },
            |row| row.get::<_, i64>(0),
        )?;
        Ok(commit_id)
    }

    pub fn insert(&mut self, conn: &Connection) -> Result<i64, rusqlite::Error> {
        if self.repo_id == 0 {
            todo!()
        }

        conn.execute("BEGIN TRANSACTION", ())?;

        if self.parent_mark.is_none() {
            println!("ROOT {}", self);
        }

        let mut stmt1 = conn.prepare_cached(indoc! { r#"
        INSERT INTO
            commits(commit_hash, commit_mark, parent_mark, repo_id)
        VALUES
            (:commit_hash, :commit_mark, :parent_mark, :repo_id)
        "#})?;

        let commit_id = stmt1.insert(named_params! {
            ":commit_hash": self.commit_hash,
            ":commit_mark": self.commit_mark,
            ":parent_mark": self.parent_mark,
            ":repo_id": self.repo_id,
        })?;

        let mut stmt2 = conn.prepare_cached(indoc! {r#"
        INSERT OR IGNORE INTO
            commit_files(commit_id, file_id)
        VALUES
            (:commit_id, :file_id)
        "#})?;

        for f in self.files.iter() {
            let file_id = Self::fetch_file_id(&f, conn)?;
            stmt2.execute(named_params! {
                ":commit_id": commit_id,
                ":file_id": file_id,
            })?;
        }

        conn.execute("COMMIT", ())?;
        Ok(commit_id)
    }

    pub fn fetch_file_id(name: &str, conn: &Connection) -> Result<i64, rusqlite::Error> {
        // select first
        let mut stmt1 = conn.prepare_cached(
            r#"
            SELECT file_id FROM files WHERE name = ? LIMIT 1
            "#,
        )?;

        // then try insert
        let mut stmt2 = conn.prepare_cached(
            r#"
            INSERT INTO files(name) VALUES (?)
            "#,
        )?;

        let file_id_option = stmt1
            .query_row([name], |row| row.get::<_, i64>(0))
            .optional()?;

        let file_id = match file_id_option {
            Some(id) => id,
            None => stmt2.insert([name])?,
        };

        Ok(file_id)
    }
}

#[cfg(test)]
mod tests {
    use rusqlite::fallible_iterator::FallibleIterator;

    use super::*;

    fn some_commits(num: i64) -> Vec<Commit> {
        let mut vec_commits = Vec::new();
        for i in 0..num {
            let commit = Commit {
                commit_hash: SqlOid::from_str("b42cd71ca109b3f5ccf9e401711005feac383ed4").unwrap(),
                commit_mark: i + 1,
                parent_mark: if i == 0 { None } else { Some(i) },
                files: Vec::new(),
                depth: None,
                commit_id: None,
                repo_id: 1,
            };
            vec_commits.push(commit);
        }
        vec_commits
    }

    #[test]
    fn test_auto_depth() {
        let conn = Connection::open_in_memory().expect("open sqlite in memory");
        conn.execute_batch(include_str!("init.sql")).expect("init");

        let num = 50;

        let mut commits = some_commits(num);

        for (index, value) in commits.iter_mut().enumerate() {
            let commit_id = value.insert(&conn).unwrap();
            assert_eq!(commit_id, (index + 1) as i64); // commit_id increase from 1

            let commit = Commit::find_by_id(commit_id, &conn).unwrap();
            assert_eq!(commit.depth, Some(commit_id - 1)); // depth increase from 0
        }
    }

    fn ancestor_sequence(num: i64) -> Vec<i64> {
        let mut results = Vec::new();
        let mut n = 0;

        loop {
            let pow2 = 1 << n;
            let current = num - pow2;

            if current > 0 {
                results.push(current);
                n += 1;
            } else {
                break;
            }
        }

        results
    }

    #[test]
    fn test_auto_ancestors() {
        let conn = Connection::open_in_memory().expect("open sqlite in memory");
        conn.execute_batch(include_str!("init.sql")).expect("init");

        let num = 5000;

        let mut commits = some_commits(num);
        for value in commits.iter_mut() {
            value.insert(&conn).unwrap();
        }

        let mut stmt = conn
            .prepare(
                r#"
                SELECT ancestor_id FROM ancestors WHERE commit_id = ? ORDER BY level
                "#,
            )
            .unwrap();
        let rows = stmt.query([num]).unwrap();
        let ancestors: Vec<i64> = rows.map(|r| r.get::<_, i64>(0)).collect().unwrap();

        assert_eq!(ancestors, ancestor_sequence(num));
    }

    #[test]
    fn test_file_hashmap() {
        let conn = Connection::open_in_memory().expect("open sqlite in memory");
        conn.execute_batch(include_str!("init.sql")).expect("init");

        let file = "README.md";

        let id_1 = Commit::fetch_file_id(file, &conn).expect("insert *1");
        assert_eq!(id_1, 1);
        let id_2 = Commit::fetch_file_id(file, &conn).expect("insert *2");
        assert_eq!(id_2, 1);
    }
}
