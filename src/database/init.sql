CREATE TABLE IF NOT EXISTS repositories (
	repo_id integer PRIMARY KEY AUTOINCREMENT,
	name text UNIQUE NOT NULL
) STRICT;

CREATE TABLE IF NOT EXISTS commits (
	commit_id integer PRIMARY KEY AUTOINCREMENT,
	commit_hash text NOT NULL,
	commit_mark integer NOT NULL,
	parent_id integer,
	depth integer,
	repo_id integer NOT NULL,
	-- UNIQUE (repo_id, parent_id, commit_id),	-- it's faster without this line
	UNIQUE (repo_id, commit_mark),
	UNIQUE (repo_id, commit_hash)
) STRICT;

CREATE TABLE IF NOT EXISTS ancestors (
	commit_id integer NOT NULL,
	level integer NOT NULL,
	ancestor_id integer,
	PRIMARY KEY (commit_id, level)
) WITHOUT ROWID, STRICT;

CREATE TRIGGER IF NOT EXISTS trigger_insert_ancestor
AFTER INSERT ON commits
FOR EACH ROW
BEGIN
INSERT INTO ancestors (commit_id, level, ancestor_id)
WITH RECURSIVE skip_cte (commit_id, level, ancestor_id) AS (
	SELECT
		commit_id,
		0 AS level,
		parent_id AS ancestor_id
	FROM
		commits
	WHERE
		commit_id = NEW.commit_id

	UNION ALL

	SELECT
		s.commit_id,
		s.level + 1,
		a.ancestor_id
	FROM
		skip_cte AS s
		INNER JOIN
			ancestors AS a
			ON
				s.ancestor_id = a.commit_id
				AND s.level = a.level
)

SELECT
	commit_id,
	level,
	ancestor_id
FROM
	skip_cte
WHERE
	ancestor_id IS NOT NULL;
END;

CREATE TABLE IF NOT EXISTS files (
	file_id integer PRIMARY KEY AUTOINCREMENT,
	name text UNIQUE NOT NULL
) STRICT;

CREATE TABLE IF NOT EXISTS commit_files (
	commit_id integer NOT NULL,
	file_id integer NOT NULL,
	PRIMARY KEY (commit_id, file_id)
) WITHOUT ROWID, STRICT;

CREATE TABLE IF NOT EXISTS refs (
	full_name text,
	short_name text,
	commit_id integer,
	time integer,
	is_tag integer,
	repo_id integer,
	PRIMARY KEY (repo_id, full_name),
	UNIQUE (repo_id, is_tag, short_name)
) WITHOUT ROWID, STRICT;

CREATE INDEX IF NOT EXISTS idx_refs_time ON refs (time);
