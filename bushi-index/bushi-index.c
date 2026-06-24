#include <sqlite3.h>
#include <stdio.h>
#include <unistd.h>

#define USE_THE_REPOSITORY_VARIABLE
#include "git-compat-util.h"

#include "config.h"
#include "path.h"
#include "repository.h"
#include "setup.h"
#include "strbuf.h"
#include "version.h"

static bool debug = false;

#define dbg(FMT, ...)                                                          \
	do {                                                                   \
		if (debug) {                                                   \
			fprintf(stderr, "[debug] %-4d " FMT "\n",              \
				__LINE__ __VA_OPT__(, ) __VA_ARGS__);          \
		}                                                              \
	} while (0)

#define err(FMT, ...)                                                          \
	do {                                                                   \
		fprintf(stderr,                                                \
			"error: " FMT "\n" __VA_OPT__(, ) __VA_ARGS__);        \
	} while (0)

char *
gitdir_from_path(const char *path)
{
	struct strbuf suspect = STRBUF_INIT;
	const char *gitdir = NULL;
	char *result = NULL;

	if (!path || !*path)
		return NULL;

	strbuf_addstr(&suspect, path);
	strbuf_strip_suffix(&suspect, "/");

	gitdir = resolve_gitdir(suspect.buf);

	if (!gitdir) {
		strbuf_addstr(&suspect, "/.git");
		gitdir = resolve_gitdir(suspect.buf);
	}

	if (gitdir)
		result = xstrdup(gitdir);

	strbuf_release(&suspect);
	return result;
}

char *
name_from_path(const char *path)
{
	struct strbuf buf = STRBUF_INIT;
	const char *base, *slash;

	if (!path || !*path)
		return NULL;

	strbuf_addstr(&buf, path);

	strbuf_strip_suffix(&buf, "/");
	strbuf_strip_suffix(&buf, "/.git");
	strbuf_strip_suffix(&buf, ".git");

	slash = strrchr(buf.buf, '/');
	base = slash ? slash + 1 : buf.buf;

	return xstrdup(base);
}

#define SQL(...) #__VA_ARGS__

enum {
	STMT_INSERT_REPOSITORY,
	STMT_GET_REPOSITORY_BY_PATH,
	STMT_GET_REPOSITORY_BY_NAME,
	STMT_DELETE_REPOSITORY,
	STMT_LIST_REPOSITORIES,

	STMT_GET_COMMIT_ID,
	STMT_INSERT_COMMIT,

	STMT_GET_FILE_ID,
	STMT_INSERT_FILE,

	STMT_INSERT_CHANGE,

	STMT_GET_REF_COMMIT_ID,
	STMT_UPSERT_REF,
	STMT_UPDATE_REF_CLEAN,

	STMT_UPDATE_REFS_DIRTY,
	STMT_DELETE_DIRTY_REFS,

	// keep COUNT the last
	STMT_COUNT
};

// clang-format off
const char *texts[STMT_COUNT] = {
	[STMT_INSERT_REPOSITORY] = SQL(
		INSERT INTO repositories
		(      repository_name
		     , repository_path
		)
		VALUES
		    (?1, ?2);
	),
	[STMT_GET_REPOSITORY_BY_PATH] = SQL(
		SELECT repository_id
		     , repository_name
		  FROM repositories
		 WHERE repository_path = ?1
		 LIMIT 1;
	),
	[STMT_GET_REPOSITORY_BY_NAME] = SQL(
		SELECT repository_id
		     , repository_path
		  FROM repositories
		 WHERE repository_name = ?1
		 LIMIT 1;
	),
	[STMT_DELETE_REPOSITORY] = SQL(
		DELETE FROM repositories
		 WHERE repository_name = ?1;
	),
	[STMT_LIST_REPOSITORIES] = SQL(
		SELECT repository_name
		     , repository_path
		  FROM repositories
		 ORDER BY repository_name;
	),
	[STMT_GET_COMMIT_ID] = SQL(
		SELECT commit_id
		  FROM commits
		 WHERE repository_id = ?1
		   AND commit_hash = ?2
		 LIMIT 1;
	),
	[STMT_INSERT_COMMIT] = SQL(
		INSERT INTO commits
		(      commit_hash
		     , parent_hash
		     , repository_id
		)
		VALUES
		    (?1, ?2, ?3);
	),
	[STMT_GET_FILE_ID] = SQL(
		SELECT file_id
		  FROM files
		 WHERE name = ?1
		 LIMIT 1;
	),
	[STMT_INSERT_FILE] = SQL(
		INSERT INTO files(name)
		VALUES
		    (?1);
	),
	[STMT_INSERT_CHANGE] = SQL(
		INSERT INTO changes
		(      commit_id
		     , file_id
		)
		VALUES
		    (?1, ?2);
	),
	[STMT_GET_REF_COMMIT_ID] = SQL(
		SELECT commit_id
		  FROM refs
		 WHERE repository_id = ?1
		   AND full_name = ?2
		 LIMIT 1;
	),
	[STMT_UPSERT_REF] = SQL(
		INSERT INTO refs
		(      full_name
		     , show_name
		     , commit_id
		     , ref_time
		     , ref_type
		     , is_dirty // always NULL here
		     , repository_id
		)
		VALUES
		    (?1, ?2, ?3, ?4, ?5, NULL, ?6)
		ON CONFLICT(repository_id, full_name)
		    DO UPDATE SET
		      show_name = excluded.show_name
		    , commit_id = excluded.commit_id
		    , ref_time = excluded.ref_time
		    , ref_type = excluded.ref_type
		    , is_dirty = NULL;
	),
	[STMT_UPDATE_REF_CLEAN] = SQL(
		UPDATE refs
		   SET is_dirty = NULL
		 WHERE repository_id = ?1
		   AND full_name = ?2;
	),
	[STMT_UPDATE_REFS_DIRTY] = SQL(
		UPDATE refs
		   SET is_dirty = 1
		 WHERE repository_id = ?1;
	),
	[STMT_DELETE_DIRTY_REFS] = SQL(
		DELETE FROM refs
		 WHERE repository_id = ?1
		   AND is_dirty IS NOT NULL;
	),
};
// clang-format on

static sqlite3 *conn = NULL;
static sqlite3_stmt *stmts[STMT_COUNT];

static sqlite3 *
db_open(const char *path)
{
	sqlite3 *db = NULL;

	dbg("opening database: %s", path);

	int rc = sqlite3_open_v2(
	    path, &db, SQLITE_OPEN_READWRITE | SQLITE_OPEN_CREATE, NULL);
	if (rc != SQLITE_OK) {
		err("cannot open database '%s': %s", path, sqlite3_errmsg(db));
		sqlite3_close(db);
		return NULL;
	}

	dbg("database opened, initializing schema");

	const char schema[] = {
#embed "init.sql"
	    , '\0'};

	char *errmsg = NULL;
	rc = sqlite3_exec(db, schema, NULL, NULL, &errmsg);
	if (rc != SQLITE_OK) {
		err("cannot initialize database '%s': %s", path,
		    errmsg ? errmsg : sqlite3_errmsg(db));
		sqlite3_free(errmsg);
		sqlite3_close(db);
		return NULL;
	}

	dbg("schema initialized, preparing %d statements", STMT_COUNT);

	for (int i = 0; i < STMT_COUNT; i++) {
		rc = sqlite3_prepare_v2(db, texts[i], -1, &stmts[i], NULL);
		if (rc != SQLITE_OK) {
			err("cannot prepare statement %2d: %s", i,
			    sqlite3_errmsg(db));
			sqlite3_close(db);
			return NULL;
		}
	}

	dbg("database ready");
	return db;
}

static void
db_close(void)
{
	if (conn == NULL) {
		dbg("database already closed");
		return;
	}

	for (int i = 0; i < STMT_COUNT; i++) {
		sqlite3_finalize(stmts[i]);
	}

	sqlite3_close(conn);
	conn = NULL;

	dbg("database closed");
}

static void
db_exec(const char *sql)
{
	char *errmsg = NULL;
	int rc;

	if (conn == NULL) {
		err("database connection is not open");
		exit(1);
	}

	rc = sqlite3_exec(conn, sql, NULL, NULL, &errmsg);

	if (rc != SQLITE_OK) {
		err("sql execution failed: %s",
		    errmsg ? errmsg : sqlite3_errmsg(conn));
		sqlite3_free(errmsg);
		exit(1);
	}
}

void
db_begin_transaction(void)
{
	db_exec("BEGIN TRANSACTION");
}

void
db_end_transaction(void)
{
	db_exec("COMMIT");
}

static void
print_usage(FILE *stream, const char *prog)
{
	fprintf(stream,
		"Usage: %s [-t DATABASE] [OPTIONS] NAME\n"
		"\n"
		"Index git repository metadata into an SQLite database.\n"
		"\n"
		"\t-a PATH       Add a repository from PATH\n"
		"\t-t DATABASE   SQLite database path\n"
		"\t-c            Check repository consistency\n"
		"\t-f            Fix missing objects\n"
		"\t-s            Show repository status\n"
		"\t-r            Remove a repository from the index\n"
		"\t-l            List indexed repositories\n"
		"\t-d            Enable debug output\n"
		"",
		prog);
}

enum Mode {
	MODE_SYNC,   // default
	MODE_ADD,    // -a PATH
	MODE_CHECK,  // -c
	MODE_FIXUP,  // -f
	MODE_STATUS, // -s
	MODE_REMOVE, // -r
	MODE_LIST,   // -l
};

void
run_list(void)
{
	sqlite3_stmt *stmt = stmts[STMT_LIST_REPOSITORIES];

	int rc;

	while ((rc = sqlite3_step(stmt)) == SQLITE_ROW) {
		const char *name = (const char *)sqlite3_column_text(stmt, 0);
		const char *path = (const char *)sqlite3_column_text(stmt, 1);

		printf("%-32.32s %s\n", name, path);
	}

	if (rc != SQLITE_DONE)
		err("failed to list repositories: %s", sqlite3_errmsg(conn));

	// generally speaking, it would only be used once.
	sqlite3_reset(stmt);
}

static char *
value_from_config(const char *key)
{
	char *config_path = repo_common_path(the_repository, "config");
	char *value = NULL;
	struct config_set set;

	git_configset_init(&set);
	git_configset_add_file(&set, config_path);
	git_configset_get_string(&set, key, &value);
	git_configset_clear(&set);

	free(config_path);
	return value;
}

void
run_add(const char *path)
{
	char *gitdir = NULL;
	char *name = NULL;

	// 1. check if path is a git repository
	gitdir = gitdir_from_path(path);
	if (!gitdir) {
		err("not a git repository: %s", path);
		goto out;
	}

	// 2. initialize the_repository to read config
	if (repo_init(the_repository, gitdir, path) < 0) {
		err("cannot initialize repository: %s", path);
		goto out;
	}

	// 3. git config --local --get bushi.name
	name = value_from_config("bushi.name");
	if (name)
		dbg("bushi.name from config: %s", name);

	// 4. if not set, derive name from path
	if (!name || !*name) {
		free(name);
		name = name_from_path(path);
		dbg("bushi.name derived from path: %s", name);
	}

	if (!name) {
		err("cannot determine repository name for: %s", path);
		goto out;
	}

	// 5. check if name already exists
	sqlite3_stmt *stmt = stmts[STMT_GET_REPOSITORY_BY_NAME];
	sqlite3_reset(stmt);
	sqlite3_bind_text(stmt, 1, name, -1, SQLITE_STATIC);
	int rc = sqlite3_step(stmt);
	if (rc == SQLITE_ROW) {
		const char *existing_path =
		    (const char *)sqlite3_column_text(stmt, 1);
		printf("repository '%s' already exists at: %s\n", name,
		       existing_path);
		goto out;
	}

	// 6. insert repository metadata into database
	stmt = stmts[STMT_INSERT_REPOSITORY];
	sqlite3_reset(stmt);
	sqlite3_bind_text(stmt, 1, name, -1, SQLITE_STATIC);
	sqlite3_bind_text(stmt, 2, gitdir, -1, SQLITE_STATIC);

	rc = sqlite3_step(stmt);
	if (rc != SQLITE_DONE)
		err("failed to add repository: %s", sqlite3_errmsg(conn));

out:
	free(name);
	free(gitdir);
}

void
run_sync(const char *name)
{
	// 1. check if name exists in database

	// 2. check if repository path exists and is a git repository

	// 3. mark all refs as dirty

	// 4. iterate refs (branches and tags)
	// 4.1 mark the ref as clean
	// 4.2 walk the commit history from the ref

	// 5. delete all dirty refs

	// 6. summary of changes
}

int
main(int argc, char *const argv[])
{
	const char *path = NULL;
	const char *name = NULL;
	const char *database = NULL;
	int i = 0;
	enum Mode mode = MODE_SYNC;

	while ((i = getopt(argc, argv, "a:t:c:f:s:r:ldhv")) != -1) {
		switch (i) {
		case 'a':
			path = optarg;
			mode = MODE_ADD;
			break;
		case 't':
			database = optarg;
			break;
		case 'c':
			mode = MODE_CHECK;
			break;
		case 'f':
			mode = MODE_FIXUP;
			break;
		case 's':
			mode = MODE_STATUS;
			break;
		case 'r':
			mode = MODE_REMOVE;
			break;
		case 'l':
			mode = MODE_LIST;
			break;
		case 'd':
			debug = true;
			break;
		case 'v':
			printf("libgit.a: %s\n", git_version_string);
			return 0;
		case 'h':
			print_usage(stdout, argv[0]);
			return 0;
		default:
			err("Unknown option: %c", i);
			print_usage(stderr, argv[0]);
			return 1;
		}
	}

	if (database == NULL) {
		database = getenv("BUSHI_DATABASE");
	}
	if (database == NULL) {
		err("database path not specified");
		return 1;
	}

	if (mode == MODE_ADD) {
		if (path == NULL) {
			err("-a requires PATH");
			return 1;
		}
		if (argv[optind] != NULL) {
			err("-a does not take NAME");
			return 1;
		}
	} else if (mode == MODE_LIST) {
		if (argv[optind] != NULL) {
			err("-l does not take arguments");
			return 1;
		}
	} else {
		if (argv[optind] == NULL || argv[optind + 1] != NULL) {
			err("exactly one NAME required");
			return 1;
		}
		name = argv[optind];
	}

	conn = db_open(database);
	if (!conn)
		return 1;

	switch (mode) {
	case MODE_LIST:
		run_list();
		break;
	case MODE_ADD:
		run_add(path);
		break;
	case MODE_SYNC:
		run_sync(name);
		break;
	default:
		err("mode not implemented yet");
		break;
	}

	// initialize_repository(the_repository);

	db_close();
	return 0;
}
