use rusqlite::types::ToSqlOutput;
use rusqlite::{types::FromSql, types::FromSqlError, types::ToSql, types::ValueRef};

#[derive(Debug, Clone)]
pub struct SqlOid(pub git2::Oid);

impl SqlOid {
    pub fn from_str(s: &str) -> Result<Self, git2::Error> {
        git2::Oid::from_str(s).map(SqlOid)
    }
}

impl From<git2::Oid> for SqlOid {
    fn from(oid: git2::Oid) -> Self {
        SqlOid(oid)
    }
}

impl From<SqlOid> for git2::Oid {
    fn from(sql_oid: SqlOid) -> Self {
        sql_oid.0
    }
}

impl std::ops::Deref for SqlOid {
    type Target = git2::Oid;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl ToSql for SqlOid {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput> {
        Ok(ToSqlOutput::from(self.0.to_string()))
    }
}

impl FromSql for SqlOid {
    fn column_result(value: ValueRef<'_>) -> rusqlite::types::FromSqlResult<Self> {
        match value.as_str() {
            Ok(s) => git2::Oid::from_str(s)
                .map(SqlOid)
                .map_err(|_| FromSqlError::InvalidType),
            Err(_) => Err(FromSqlError::InvalidType),
        }
    }
}
