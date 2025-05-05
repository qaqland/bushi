CREATE TABLE IF NOT EXISTS repositories (
    repo_id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT UNIQUE NOT NULL
) STRICT ;

CREATE TABLE IF NOT EXISTS commits (
    commit_id INTEGER PRIMARY KEY AUTOINCREMENT,
    commit_hash TEXT NOT NULL,
    commit_mark INTEGER NOT NULL,
    parent_mark INTEGER,
    depth INTEGER DEFAULT 0,
    repo_id INTEGER NOT NULL
) STRICT ;

CREATE INDEX IF NOT EXISTS idx_commit_hash ON commits(repo_id, commit_hash) ;
CREATE INDEX IF NOT EXISTS idx_commit_mark ON commits(repo_id, commit_mark) ;
CREATE INDEX IF NOT EXISTS idx_parent_mark ON commits(repo_id, parent_mark) ;

CREATE TABLE IF NOT EXISTS ancestors (
    commit_id INTEGER NOT NULL,
    level INTEGER NOT NULL,
    ancestor_id INTEGER,
    PRIMARY KEY (commit_id, level)
) WITHOUT ROWID, STRICT ;

CREATE TRIGGER IF NOT EXISTS auto_depth_and_ancestors
AFTER INSERT ON commits
FOR EACH ROW
WHEN NEW.parent_mark IS NOT NULL
BEGIN
    UPDATE commits
    SET depth = (
        SELECT
            depth + 1
        FROM
            commits
        WHERE
            repo_id = NEW.repo_id
            AND commit_mark = NEW.parent_mark
    )
    WHERE
        commit_id = NEW.commit_id ;

    INSERT INTO
        ancestors (commit_id, level, ancestor_id)
    WITH RECURSIVE skip_cte (commit_id, level, ancestor_id) AS (
    SELECT
        NEW.commit_id,
        0 AS level,
        c.commit_id AS ancestor_id
        FROM
            commits AS c
        WHERE
            repo_id = NEW.repo_id
            AND commit_mark = NEW.parent_mark

    UNION ALL

    SELECT
        s.commit_id, s.level + 1, a.ancestor_id
    FROM
        skip_cte AS s
    INNER JOIN
        ancestors AS a
    ON
        s.ancestor_id = a.commit_id
        AND s.level = a.level
    )

    SELECT
        commit_id, level, ancestor_id
    FROM
        skip_cte
    WHERE
        ancestor_id IS NOT NULL;
END ;

CREATE TABLE IF NOT EXISTS files (
    file_id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT UNIQUE NOT NULL
) STRICT ;

CREATE TABLE IF NOT EXISTS commit_files (
    commit_id INTEGER NOT NULL,
    file_id INTEGER NOT NULL,
    PRIMARY KEY (commit_id, file_id)
) WITHOUT ROWID, STRICT ;

CREATE TABLE IF NOT EXISTS refs (
    full_name TEXT,
    short_name TEXT,
    commit_id INTEGER,
    time INTEGER,
    is_tag INTEGER,
    repo_id INTEGER,
    PRIMARY KEY (repo_id, full_name),
    UNIQUE (repo_id, is_tag, short_name)
) WITHOUT ROWID, STRICT ;

CREATE INDEX IF NOT EXISTS idx_refs_time ON refs(time) ;
