use indoc::indoc;
use rusqlite::{named_params, Connection};

use super::{Commit, SqlOid};

pub struct Reference {
    pub full_name: String,
    pub short_name: String,
    pub commit_id: i64,
    pub commit_hash: SqlOid,
    pub time: i64,
    pub repo_id: i64,
    pub is_tag: bool,
}

impl Reference {
    pub fn upsert(&mut self, conn: &Connection) -> Result<(), rusqlite::Error> {
        if self.repo_id == 0 {
            todo!()
        }
        if self.commit_id == 0 {
            self.commit_id = Commit::get_id_by_hash(self.repo_id, &self.commit_hash, conn)?;
        }
        let mut stmt = conn.prepare_cached(indoc! { r#"
            INSERT INTO refs (
                full_name,
                short_name,
                commit_id,
                time,
                repo_id,
                is_tag
            ) VALUES (
                :full_name, :short_name, :commit_id, :time, :repo_id, :is_tag
            )
            ON CONFLICT(repo_id, full_name) DO UPDATE SET
                short_name = excluded.short_name,
                commit_id = excluded.commit_id,
                time = excluded.time,
                is_tag = excluded.is_tag
        "#})?;
        stmt.execute(named_params! {
            ":full_name": self.full_name,
            ":short_name": self.short_name,
            ":commit_id": self.commit_id,
            ":time": self.time,
            ":repo_id": self.repo_id,
            ":is_tag": self.is_tag,
        })?;
        Ok(())
    }
}
