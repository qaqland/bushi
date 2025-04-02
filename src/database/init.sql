CREATE TABLE IF NOT EXISTS repositories (
	repo_id integer PRIMARY KEY AUTOINCREMENT,
	name text UNIQUE NOT NULL
) STRICT;

CREATE TABLE IF NOT EXISTS commits (
	commit_id integer PRIMARY KEY AUTOINCREMENT,
	commit_hash text NOT NULL,
	commit_mark integer NOT NULL,
	parent_id integer,
	repo_id integer NOT NULL,
	-- UNIQUE (repo_id, parent_id, commit_id),	-- it's faster without this line
	UNIQUE (repo_id, commit_mark),
	UNIQUE (repo_id, commit_hash)
) STRICT;

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
