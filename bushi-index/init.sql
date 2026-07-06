-- It's use to initialize the SQLite database. We don't have migration for now.
-- Regenerating the database won't cause any data loss.

-- https://sqlite.org/pragma.html#pragma_synchronous
PRAGMA synchronous = OFF;

CREATE TABLE IF NOT EXISTS repositories
(      repository_id    INTEGER PRIMARY KEY AUTOINCREMENT
     , repository_name  TEXT    UNIQUE NOT NULL -- display on website
     , repository_path  TEXT    UNIQUE NOT NULL -- alias GIT_DIR
     , repository_head  TEXT                    -- default branch
) STRICT;

CREATE TABLE IF NOT EXISTS commits
(      commit_id        INTEGER PRIMARY KEY AUTOINCREMENT
     , commit_hash      TEXT    NOT NULL
     , parent_hash      TEXT                    -- only first parent
     , first_depth      INTEGER                 -- only first parent
     , repository_id    INTEGER NOT NULL
) STRICT;

CREATE TABLE IF NOT EXISTS ancestors
(      commit_id    INTEGER NOT NULL
     , exponent     INTEGER NOT NULL
     , ancestor_id  INTEGER NOT NULL
     , PRIMARY KEY (commit_id, exponent)
) WITHOUT ROWID, STRICT;

CREATE TRIGGER IF NOT EXISTS trg_commits_first_depth_ancestors
AFTER UPDATE OF first_depth ON commits
WHEN NEW.parent_hash IS NOT NULL
BEGIN
    INSERT OR IGNORE INTO ancestors (commit_id, exponent, ancestor_id)
    WITH RECURSIVE skip_list(commit_id, exponent, ancestor_id) AS (
        SELECT NEW.commit_id,
               0,
               parent.commit_id
          FROM commits AS parent
         WHERE parent.repository_id = NEW.repository_id
           AND parent.commit_hash = NEW.parent_hash

        UNION ALL

        SELECT s.commit_id,
               s.exponent + 1,
               a.ancestor_id
          FROM skip_list AS s
          JOIN ancestors AS a
            ON a.commit_id = s.ancestor_id
           AND a.exponent = s.exponent
    )
    SELECT commit_id, exponent, ancestor_id
      FROM skip_list;
END;

CREATE INDEX IF NOT EXISTS idx_commit_hash
    ON commits (
       repository_id
     , commit_hash
       );

CREATE INDEX IF NOT EXISTS idx_parent_hash
    ON commits (
       repository_id
     , parent_hash
       );

CREATE INDEX IF NOT EXISTS idx_commit_hash_only
    ON commits (
       commit_hash
       );

CREATE TABLE IF NOT EXISTS files
(      file_id          INTEGER PRIMARY KEY AUTOINCREMENT
     , name             TEXT    UNIQUE NOT NULL -- just like the hashmap
) STRICT;

CREATE TABLE IF NOT EXISTS changes
(      commit_id        INTEGER NOT NULL
     , file_id          INTEGER NOT NULL
     , last_commit_id   INTEGER          -- previous commit that modified this file
     , PRIMARY KEY (commit_id, file_id)
     , FOREIGN KEY (last_commit_id) REFERENCES commits(commit_id)
) WITHOUT ROWID, STRICT;

CREATE INDEX IF NOT EXISTS idx_changes_file_last
    ON changes (
       file_id
     , last_commit_id
       );

-- 查询 git log -- path 时我们可以直接使用 JOIN 和 files.name like
-- 'path%'， 也可以先把相关的 file_id 查出来再用 IN
-- 进行查询，后者可能会更快一些？但是命中 file_id
-- 的数量可能会比较多，IN 语句存在不确定性。做 git blame
-- 使用方法二更合适。

CREATE TABLE IF NOT EXISTS refs
(      full_name        TEXT    NOT NULL  -- e.g. refs/heads/fix/issue-1
     , show_name        TEXT    NOT NULL  -- e.g. fix:issue-1
     , commit_id        INTEGER NOT NULL  -- always commit_id
     , ref_time         INTEGER NOT NULL  -- commit timestamp
     , ref_type         INTEGER NOT NULL  -- 0 is branch, 1 is tag
     , is_dirty         INTEGER DEFAULT NULL
     , repository_id    INTEGER NOT NULL
     , PRIMARY KEY (repository_id, full_name)
     , UNIQUE (repository_id, ref_type, show_name)
) WITHOUT ROWID, STRICT;

CREATE INDEX IF NOT EXISTS idx_refs_time
    ON refs (
       repository_id
     , ref_time
       );

CREATE INDEX IF NOT EXISTS idx_refs_dirty
    ON refs (
       repository_id
     , is_dirty
       )
 WHERE is_dirty IS NOT NULL;

-- vim: set expandtab ts=4:
