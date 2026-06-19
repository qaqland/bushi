#define _GNU_SOURCE

#include <assert.h>
#include <errno.h>
#include <git2.h>
#include <git2/commit.h>
#include <git2/config.h>
#include <git2/errors.h>
#include <git2/oid.h>
#include <git2/refs.h>
#include <git2/repository.h>
#include <git2/types.h>
#include <limits.h>
#include <spawn.h>
#include <sqlite3.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <sys/wait.h>
#include <unistd.h>

static int verbosity = 0;

#define E(FMT, ...)                                                            \
	do {                                                                   \
		fprintf(stderr, "[E] %-4d " FMT "\n",                          \
			__LINE__ __VA_OPT__(, ) __VA_ARGS__);                  \
		exit(1);                                                       \
	} while (0)

#define I(FMT, ...)                                                            \
	do {                                                                   \
		fprintf(stdout, "[I] " FMT "\n" __VA_OPT__(, ) __VA_ARGS__);   \
	} while (0)

#define D(FMT, ...)                                                            \
	do {                                                                   \
		if (verbosity >= 1) {                                          \
			fprintf(stderr, "[D] %-4d " FMT "\n",                  \
				__LINE__ __VA_OPT__(, ) __VA_ARGS__);          \
		}                                                              \
	} while (0)

#define T(FMT, ...)                                                            \
	do {                                                                   \
		if (verbosity >= 2) {                                          \
			fprintf(stderr, "[T] %-4d " FMT "\n",                  \
				__LINE__ __VA_OPT__(, ) __VA_ARGS__);          \
		}                                                              \
	} while (0)

#define SQL(...) #__VA_ARGS__

#if defined(__GNUC__) || defined(__clang__)
#define likely(x) __builtin_expect(!!(x), 1)
#define unlikely(x) __builtin_expect(!!(x), 0)
#else
#define likely(x) (x)
#define unlikely(x) (x)
#endif

enum {
	STMT_UPSERT_REPOSITORY,
	STMT_GET_REPOSITORY_ID,
	STMT_DELETE_REPOSITORY,

	STMT_GET_COMMIT_ID,
	STMT_INSERT_COMMIT,

	STMT_GET_FILE_ID,
	STMT_INSERT_FILE,

	STMT_INSERT_CHANGE,
	STMT_UPDATE_GENERATION,

	STMT_GET_REF_COMMIT_ID,
	STMT_UPSERT_REF,
	STMT_UPDATE_REF_CLEAN,

	STMT_UPDATE_REFS_DIRTY,
	STMT_DELETE_DIRTY_REFS,

	// keep COUNT the last
	STMT_COUNT
};

enum {
	REF_TYPE_NULL = 0,
	REF_TYPE_BRANCH = 1,
	REF_TYPE_TAG = 2,
};

// clang-format off
const char *texts[STMT_COUNT] = {
	[STMT_UPSERT_REPOSITORY] = SQL(
		INSERT INTO repositories
		(      repository_name
		     , repository_path
		)
		VALUES
		    (?1, ?2)
		ON CONFLICT(repository_name)
		    DO UPDATE SET
		      repository_path = excluded.repository_path;
	),
	[STMT_GET_REPOSITORY_ID] = SQL(
		SELECT repository_id
		  FROM repositories
		 WHERE repository_name = ?1
		 LIMIT 1;
	),
	[STMT_DELETE_REPOSITORY] = SQL(
		DELETE FROM repositories
		 WHERE repository_name = ?1;
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
		     , generation
		     , repository_id
		)
		VALUES
		    (?1, ?2, ?3, ?4);
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
	[STMT_UPDATE_GENERATION] = SQL(
		UPDATE commits
		   SET generation = parent.generation + 1
		  FROM commits AS parent
		 WHERE commits.commit_id = ?1
		   AND parent.generation IS NOT NULL
		   AND parent.commit_hash = commits.parent_hash
		   AND parent.repository_id = commits.repository_id;
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

static sqlite3_stmt *stmts[STMT_COUNT];
static sqlite3 *connection;
static int64_t repository_id;
static git_repository *repository_git;

static size_t
str_with_sfx(const char *str, const char *sfx)
{
	assert(sfx);

	if (!str) {
		return 0;
	}
	size_t str_len = strlen(str);
	size_t sfx_len = strlen(sfx);

	if (sfx_len > str_len) {
		return 0;
	}
	if (strcmp(str + str_len - sfx_len, sfx) == 0) {
		return sfx_len;
	}
	return 0;
}

static const char *
name_from_path(const char *path)
{
	assert(path);
	assert(path[0] == '/');

	const char *end = path + strlen(path);
	char *suffixes[] = {"/.git", ".git", NULL};

	for (int i = 0; suffixes[i]; i++) {
		size_t try_len = str_with_sfx(path, suffixes[i]);
		if (try_len) {
			end -= try_len;
			break;
		}
	}

	const char *start = end;
	while (start > path) {
		if (*(start - 1) == '/') {
			break;
		}
		start--;
	}

	size_t len = end - start;
	if (len == 0) {
		return NULL;
	}

	static char name[NAME_MAX + 1];
	memcpy(name, start, len);
	name[len] = '\0';
	return name;
}

static void
db_exec(const char *sql)
{
	if (unlikely(!connection)) {
		E("exec called without a database connection");
	}

	if (sql == NULL || sql[0] == '\0') {
		E("empty SQL statement");
	}

	D("execute sql: %s", sql);

	char *errmsg = NULL;
	int rc = sqlite3_exec(connection, sql, NULL, NULL, &errmsg);
	if (rc != SQLITE_OK) {
		E("exec error: %s [sql: %s]", errmsg, sql);
	}
}

static void
db_begin_transaction(void)
{
	db_exec("BEGIN TRANSACTION");
}

static void
db_end_transaction(void)
{
	db_exec("COMMIT");
}

static void
db_setup(const char *path)
{
	sqlite3 *conn = NULL;
	char *errmsg = NULL;
	int rc;

	assert(path);
	assert(!connection);

	I("prepare database %s", path);

	rc = sqlite3_open(path, &conn);
	if (rc != SQLITE_OK) {
		E("open database '%s' failed: %s", path, sqlite3_errmsg(conn));
	}

	const char init_sql[] = {
#embed "init.sql"
	    , '\0'};

	D("prepare database schema");
	rc = sqlite3_exec(conn, init_sql, NULL, NULL, &errmsg);
	if (rc != SQLITE_OK) {
		E("initialize schema failed: %s", errmsg);
	}

	D("prepare SQL statements");
	for (int i = 0; i < STMT_COUNT; i++) {
		D("prepare statement %2d: %.32s...", i, texts[i]);
		rc = sqlite3_prepare_v2(conn, texts[i], -1, &stmts[i], NULL);
		if (rc != SQLITE_OK) {
			E("prepare statement %2d failed: %s", i,
			  sqlite3_errmsg(conn));
		}
	}

	D("prepare %d statements", STMT_COUNT);
	connection = conn;
}

static void
db_teardown(void)
{
	for (int i = 0; i < STMT_COUNT; i++) {
		sqlite3_finalize(stmts[i]);
	}
	sqlite3_close(connection);
}

// ROWID is always not zero
// remember to update head after scanning branches
static void
db_sync_repository_id(const char *name, const char *path)
{
	sqlite3_stmt *stmt = NULL;
	int64_t id = 0;
	int rc;

	assert(name);
	assert(path);
	assert(repository_id == 0);

	D("upsert repository %s at %s", name, path);

	stmt = stmts[STMT_UPSERT_REPOSITORY];
	sqlite3_reset(stmt);
	sqlite3_clear_bindings(stmt);

	T("bind upsert repository name=%s path=%s", name, path);
	sqlite3_bind_text(stmt, 1, name, -1, SQLITE_STATIC);
	sqlite3_bind_text(stmt, 2, path, -1, SQLITE_STATIC);

	rc = sqlite3_step(stmt);
	if (rc != SQLITE_DONE) {
		E("%s", sqlite3_errmsg(connection));
	}

	stmt = stmts[STMT_GET_REPOSITORY_ID];
	sqlite3_reset(stmt);
	sqlite3_clear_bindings(stmt);

	T("bind get repository_id name=%s", name);
	sqlite3_bind_text(stmt, 1, name, -1, SQLITE_STATIC);

	rc = sqlite3_step(stmt);
	if (rc != SQLITE_ROW) {
		E("%s", sqlite3_errmsg(connection));
	}

	id = sqlite3_column_int64(stmt, 0);
	T("got repository_id %ld", id);
	D("set repository_id: %ld", id);
	repository_id = id;
}

static void
db_delete_repository(const char *name)
{
	sqlite3_stmt *stmt = stmts[STMT_DELETE_REPOSITORY];

	assert(name);
	assert(repository_id == 0);

	sqlite3_reset(stmt);
	sqlite3_clear_bindings(stmt);

	D("delete repository %s", name);

	T("bind delete repository name=%s", name);
	sqlite3_bind_text(stmt, 1, name, -1, SQLITE_STATIC);
	int rc = sqlite3_step(stmt);
	if (rc != SQLITE_DONE) {
		E("%s", sqlite3_errmsg(connection));
		return;
	}
	int count = sqlite3_changes(connection);
	T("deleted %d rows", count);
}

static void
git_teardown(void)
{
	git_repository_free(repository_git);
	git_libgit2_shutdown();
}

static void
git_setup(const char *git_dir)
{
	git_repository *repo_git;
	int rc = 0;

	assert(git_dir);

	git_libgit2_init();
	git_libgit2_opts(GIT_OPT_ENABLE_CACHING, false);
	git_libgit2_opts(GIT_OPT_ENABLE_STRICT_HASH_VERIFICATION, false);

	I("open GIT_DIR %s", git_dir);
	setenv("GIT_DIR", git_dir, 1);

	rc = git_repository_open_bare(&repo_git, git_dir);
	if (rc < 0) {
		E("%s", git_error_last()->message);
	}

	git_config *config;
	rc = git_repository_config_snapshot(&config, repo_git);
	if (rc < 0) {
		E("%s", git_error_last()->message);
	}

	// head is the same, but update it after we scan all branches
	const char *name = NULL;
	rc = git_config_get_string(&name, config, "bushi.name");
	if (rc < 0 && rc != GIT_ENOTFOUND) {
		E("%s", git_error_last()->message);
	}

	if (!name || !name[0]) {
		name = name_from_path(git_dir);
	}
	if (!name) {
		E("cannot derive repository name from %s", git_dir);
	}
	db_sync_repository_id(name, git_dir);

	repository_git = repo_git;
	git_config_free(config);
}

static void
db_update_ref_clean(const char *full_name)
{
	sqlite3_stmt *stmt = stmts[STMT_UPDATE_REF_CLEAN];

	assert(full_name);

	D("mark ref clean: %s", full_name);

	sqlite3_reset(stmt);
	sqlite3_clear_bindings(stmt);
	T("bind ref clean repository_id=%ld full_name=%s", repository_id,
	  full_name);
	sqlite3_bind_int64(stmt, 1, repository_id);
	sqlite3_bind_text(stmt, 2, full_name, -1, SQLITE_STATIC);
	int rc = sqlite3_step(stmt);
	if (rc != SQLITE_DONE) {
		E("%s", sqlite3_errmsg(connection));
	}
	T("marked ref clean");
}

static void
db_update_refs_dirty(void)
{
	sqlite3_stmt *stmt = stmts[STMT_UPDATE_REFS_DIRTY];

	D("mark all refs dirty for repository %ld", repository_id);

	sqlite3_reset(stmt);
	sqlite3_clear_bindings(stmt);
	T("bind refs dirty repository_id=%ld", repository_id);
	sqlite3_bind_int64(stmt, 1, repository_id);
	int rc = sqlite3_step(stmt);
	if (rc != SQLITE_DONE) {
		E("%s", sqlite3_errmsg(connection));
	}
	int count = sqlite3_changes(connection);
	T("marked %d refs dirty", count);
}

static void
db_delete_dirty_refs(void)
{
	sqlite3_stmt *stmt = stmts[STMT_DELETE_DIRTY_REFS];

	D("delete dirty refs for repository %ld", repository_id);

	sqlite3_reset(stmt);
	sqlite3_clear_bindings(stmt);
	T("bind delete dirty refs repository_id=%ld", repository_id);
	sqlite3_bind_int64(stmt, 1, repository_id);
	int rc = sqlite3_step(stmt);
	if (rc != SQLITE_DONE) {
		E("%s", sqlite3_errmsg(connection));
	}
	int count = sqlite3_changes(connection);
	T("deleted %d dirty refs", count);
}

static int64_t
db_get_ref_commit(const char *full_name)
{
	sqlite3_stmt *stmt = stmts[STMT_GET_REF_COMMIT_ID];
	int64_t commit_id = 0;
	int rc;

	assert(full_name);

	D("lookup ref %s", full_name);

	sqlite3_reset(stmt);
	sqlite3_clear_bindings(stmt);

	T("bind ref commit repository_id=%ld full_name=%s", repository_id,
	  full_name);
	sqlite3_bind_int64(stmt, 1, repository_id);
	sqlite3_bind_text(stmt, 2, full_name, -1, SQLITE_STATIC);

	rc = sqlite3_step(stmt);
	switch (rc) {
	case SQLITE_ROW:
		commit_id = sqlite3_column_int64(stmt, 0);
		T("got ref commit_id %ld", commit_id);
		break;
	case SQLITE_DONE:
		T("ref commit not found");
		break;
	default:
		E("%s", sqlite3_errmsg(connection));
	}
	return commit_id;
}

static void
db_upsert_ref(const char *full_name, int64_t commit_id, int64_t ref_time)
{
	sqlite3_stmt *stmt = stmts[STMT_UPSERT_REF];

	assert(full_name);
	assert(commit_id);

	D("upsert ref %s -> commit_id %ld", full_name, commit_id);

	char *show_name = NULL;

	int ref_type = REF_TYPE_NULL;
	if (strncmp(full_name, "refs/heads/", strlen("refs/heads/")) == 0) {
		ref_type = REF_TYPE_BRANCH;
		show_name = strdup(full_name + strlen("refs/heads/"));
		D("ref is a branch");
	} else if (strncmp(full_name, "refs/tags/", strlen("refs/tags/")) ==
		   0) {
		ref_type = REF_TYPE_TAG;
		show_name = strdup(full_name + strlen("refs/tags/"));
		D("ref is a tag");
	} else {
		D("skip non-branch/tag ref %s", full_name);
		return;
	}

	for (char *ptr = show_name; *ptr; ptr++) {
		if (*ptr == '/') {
			*ptr = ':';
		}
	}

	sqlite3_reset(stmt);
	sqlite3_clear_bindings(stmt);

	T("bind upsert ref full_name=%s show_name=%s commit_id=%ld "
	  "ref_time=%ld "
	  "ref_type=%d repository_id=%ld",
	  full_name, show_name, commit_id, ref_time, ref_type, repository_id);
	sqlite3_bind_text(stmt, 1, full_name, -1, SQLITE_STATIC);
	sqlite3_bind_text(stmt, 2, show_name, -1, SQLITE_STATIC);
	sqlite3_bind_int64(stmt, 3, commit_id);
	sqlite3_bind_int64(stmt, 4, ref_time);
	sqlite3_bind_int(stmt, 5, ref_type);
	sqlite3_bind_int64(stmt, 6, repository_id);

	int rc = sqlite3_step(stmt);
	if (rc != SQLITE_DONE) {
		E("%s", sqlite3_errmsg(connection));
	}
	T("upserted ref");
	free(show_name);
}

static int64_t
db_get_file_id(const char *name)
{
	sqlite3_stmt *stmt;
	int64_t id = 0;
	int rc;

	assert(name);

	T("lookup file %s", name);

	stmt = stmts[STMT_GET_FILE_ID];
	sqlite3_reset(stmt);
	sqlite3_clear_bindings(stmt);

	sqlite3_bind_text(stmt, 1, name, -1, SQLITE_STATIC);
	rc = sqlite3_step(stmt);
	if (rc == SQLITE_ROW) {
		id = sqlite3_column_int64(stmt, 0);
		goto done;
	}

	assert(id == 0);
	stmt = stmts[STMT_INSERT_FILE];
	sqlite3_reset(stmt);
	sqlite3_clear_bindings(stmt);

	T("bind insert file name=%s", name);
	sqlite3_bind_text(stmt, 1, name, -1, SQLITE_STATIC);
	rc = sqlite3_step(stmt);
	if (rc != SQLITE_DONE) {
		E("%s", sqlite3_errmsg(connection));
		goto done;
	}
	id = sqlite3_last_insert_rowid(connection);

done:
	T("got file_id %ld", id);
	return id;
}

static void
db_insert_change(int64_t commit_id, int64_t file_id)
{
	assert(commit_id);
	assert(file_id);

	T("record change: commit %ld, file %ld", commit_id, file_id);

	sqlite3_stmt *stmt = stmts[STMT_INSERT_CHANGE];
	sqlite3_reset(stmt);
	sqlite3_clear_bindings(stmt);

	sqlite3_bind_int64(stmt, 1, commit_id);
	sqlite3_bind_int64(stmt, 2, file_id);

	int rc = sqlite3_step(stmt);
	if (rc != SQLITE_DONE) {
		E("%s", sqlite3_errmsg(connection));
	}
}

static int64_t
db_get_commit_id(const char *commit_hash)
{
	int64_t id = 0;
	int rc;

	assert(repository_id);
	assert(commit_hash);

	sqlite3_stmt *stmt = stmts[STMT_GET_COMMIT_ID];
	sqlite3_reset(stmt);
	sqlite3_clear_bindings(stmt);

	sqlite3_bind_int(stmt, 1, repository_id);
	sqlite3_bind_text(stmt, 2, commit_hash, -1, SQLITE_STATIC);
	T("lookup commit %s", commit_hash);
	rc = sqlite3_step(stmt);
	switch (rc) {
	case SQLITE_ROW:
		id = sqlite3_column_int64(stmt, 0);
		T("got commit_id %ld", id);
		break;
	case SQLITE_DONE:
		T("commit not found");
		break;
	default:
		E("%s", sqlite3_errmsg(connection));
		break;
	}

	return id;
}

static int64_t
db_insert_commit(const char *commit_hash, const char *parent_hash)
{
	assert(repository_id);
	assert(commit_hash);

	int64_t id = db_get_commit_id(commit_hash);
	if (id != 0) {
		id = 0;
		goto done;
	}

	sqlite3_stmt *stmt = stmts[STMT_INSERT_COMMIT];
	sqlite3_reset(stmt);
	sqlite3_clear_bindings(stmt);
	T("bind insert commit hash=%s parent_hash=%s repository_id=%ld",
	  commit_hash, parent_hash ? parent_hash : "NULL", repository_id);
	sqlite3_bind_text(stmt, 1, commit_hash, -1, NULL);
	if (parent_hash) {
		assert(parent_hash[0]);
		sqlite3_bind_text(stmt, 2, parent_hash, -1, NULL);
	} else {
		sqlite3_bind_null(stmt, 2);	// root commit
		sqlite3_bind_int64(stmt, 3, 0); // generation
	}
	sqlite3_bind_int64(stmt, 4, repository_id);

	int rc = sqlite3_step(stmt);
	if (rc != SQLITE_DONE) {
		E("%s", sqlite3_errmsg(connection));
		goto done;
	}
	id = sqlite3_last_insert_rowid(connection);

done:
	T("insert commit %ld", id);
	return id;
}

static void
db_update_generation(int64_t commit_id)
{
	assert(commit_id);

	sqlite3_stmt *stmt = stmts[STMT_UPDATE_GENERATION];
	sqlite3_reset(stmt);
	sqlite3_clear_bindings(stmt);

	T("update generation for commit %ld", commit_id);
	sqlite3_bind_int64(stmt, 1, commit_id);

	int rc = sqlite3_step(stmt);
	if (rc != SQLITE_DONE) {
		E("%s", sqlite3_errmsg(connection));
	}

	int rf = sqlite3_changes(connection);
	if (rf == 0) {
		T("generation already set");
	}
}

static const char *
commit_hash_from_object(const git_commit *commit)
{
	static char double_buffer[2][GIT_OID_MAX_HEXSIZE + 1];
	static char *commit_hash = double_buffer[0];
	static const git_commit *last_commit;

	if (!commit) {
		return NULL;
	}

	if (last_commit == commit) {
		return commit_hash;
	}
	last_commit = commit;
	commit_hash = double_buffer[commit_hash == double_buffer[0] ? 1 : 0];

	const git_oid *oid = git_commit_id(commit);
	git_oid_tostr(commit_hash, sizeof(double_buffer[0]), oid);
	return commit_hash;
}

static void
sync_commit_list(git_commit *commit)
{
	assert(commit);
	assert(repository_git);

	const char *commit_hash = commit_hash_from_object(commit);
	int64_t commit_id = db_get_commit_id(commit_hash);
	if (commit_id != 0) {
		return;
	}

	char *new_hash = strdup(commit_hash);
	char *old_hash = NULL;
	git_commit *walker;
	git_commit_dup(&walker, commit);

	while (true) {
		commit_hash = commit_hash_from_object(walker);
		unsigned int count = git_commit_parentcount(walker);
		git_commit *parent = NULL;
		if (count != 0) {
			git_commit_parent(&parent, walker, 0);
			// TODO only first now
			// we would like to save them and parse later
			git_commit_free(walker);
			walker = parent;
		}
		const char *parent_hash = commit_hash_from_object(parent);
		int64_t commit_id = db_insert_commit(commit_hash, parent_hash);
		if (commit_id == 0 || count == 0) {
			old_hash = count ? strdup(parent_hash) : NULL;
			break;
		}
	}
	git_commit_free(walker);
	I("scan commit range %s..%s", old_hash ? old_hash : "NULL", new_hash);

	char commit_range[GIT_OID_MAX_HEXSIZE * 2 + 3];
	if (old_hash) {
		snprintf(commit_range, sizeof(commit_range), "%s..%s", old_hash,
			 new_hash);
	} else {
		strcpy(commit_range, new_hash);
	}
	free(new_hash);
	free(old_hash);

	char *args[] = {
	    "git",
	    "log",
	    "--pretty=format:%n%H",
	    "--name-only",
	    "--first-parent",
	    "--reverse",
	    commit_range,
	    NULL,
	};
	// git log --pretty=format:%n%H --name-only --first-parent --reverse

	int pipefd[2];
	if (pipe(pipefd) < 0) {
		E("pipe failed: %s", strerror(errno));
	}

	pid_t pid = fork();
	if (pid < 0) {
		close(pipefd[0]);
		close(pipefd[1]);
		E("fork failed: %s", strerror(errno));
	}

	if (pid == 0) {
		close(pipefd[0]);
		dup2(pipefd[1], STDOUT_FILENO);
		close(pipefd[1]);
		execvp("git", args);
		E("execvp failed: %s", strerror(errno));
	}
	close(pipefd[1]);

	FILE *f = fdopen(pipefd[0], "r");
	char line_buffer[PATH_MAX + 1]; // one extra '\n'
	commit_id = 0;

	while (fgets(line_buffer, sizeof(line_buffer), f)) {
		line_buffer[strcspn(line_buffer, "\n")] = '\0';
		// done, reset
		if (line_buffer[0] == '\0') {
			if (commit_id != 0) {
				commit_id = 0;
				db_end_transaction();
			}
			continue;
		}
		// new commit
		if (commit_id == 0) {
			db_begin_transaction();
			int64_t id = db_get_commit_id(line_buffer);
			if (id == 0) {
				E("commit %s not found", line_buffer);
			}
			commit_id = id;
			db_update_generation(commit_id);
			continue;
		}
		// update commit_change_files
		int64_t file_id = db_get_file_id(line_buffer);
		db_insert_change(commit_id, file_id);
	}
	// last commit doesn't have a trailing newline
	if (commit_id != 0) {
		db_end_transaction();
	}
	fclose(f);

	close(pipefd[0]);
	int status;
	if (waitpid(pid, &status, 0) < 0) {
		E("waitpid failed: %s", strerror(errno));
	}
	if (!WIFEXITED(status) || WEXITSTATUS(status) != 0) {
		E("git log failed");
	}
}

static int
sync_reference(const char *name, void *payload)
{
	(void)payload;
	int rc;

	D("walk reference %s", name);

	if (strncmp(name, "refs/heads/", strlen("refs/heads/")) &&
	    strncmp(name, "refs/tags/", strlen("refs/tags/"))) {
		D("skip %s (not a branch or tag)", name);
		return 0;
	}

	git_reference *ref;
	rc = git_reference_lookup(&ref, repository_git, name);
	if (rc < 0) {
		E("%s", git_error_last()->message);
	}

	git_commit *target;
	rc = git_reference_peel((git_object **)&target, ref, GIT_OBJECT_COMMIT);
	if (rc < 0) {
		E("%s", git_error_last()->message);
	}
	git_reference_free(ref);
	const char *commit_hash = commit_hash_from_object(target);
	int64_t commit_id = db_get_commit_id(commit_hash);

	if (db_get_ref_commit(name) == commit_id && commit_id != 0) {
		D("skip %s (same commit)", name);
		git_commit_free(target);
		db_update_ref_clean(name);
		return 0;
	}

	sync_commit_list(target);

	int64_t timestamp = git_commit_time(target);
	int offset_in_min = git_commit_time_offset(target);
	int64_t ref_time = timestamp + offset_in_min * 60;

	db_upsert_ref(name, db_get_commit_id(commit_hash), ref_time);
	git_commit_free(target);
	return 0;
}

static void
print_usage(void)
{
	puts("usage: bushi-index -t DATABASE -p GIT_DIR\n"
	     "       bushi-index -t DATABASE -d NAME\n");
}

int
main(int argc, char **argv)
{
	char *repo_path = NULL;
	char *repo_name = NULL;
	bool is_delete = false;
	char *db_path = NULL;
	int rc;
	int opt;

	while ((opt = getopt(argc, argv, "+p:d:t:vh")) != -1) {
		switch (opt) {
		case 'p':
			repo_path = realpath(optarg, NULL);
			break;
		case 'd':
			is_delete = true;
			repo_name = strdup(optarg);
			break;
		case 't':
			db_path = strdup(optarg);
			break;
		case 'v':
			verbosity++;
			break;
		case 'h':
			print_usage();
			exit(0);
		default:
			print_usage();
			exit(1);
		}
	}

	if (optind != argc || !db_path ||
	    (is_delete ? !repo_name : !repo_path)) {
		print_usage();
		exit(1);
	}

	db_setup(db_path);

	if (is_delete) {
		db_delete_repository(repo_name);
		exit(0);
	}

	git_setup(repo_path);

	db_update_refs_dirty();

	rc = git_reference_foreach_name(repository_git, sync_reference, NULL);
	if (rc < 0) {
		E("failed to iterate references");
	}

	db_delete_dirty_refs();

	git_teardown();
	db_teardown();
	exit(0);
}
