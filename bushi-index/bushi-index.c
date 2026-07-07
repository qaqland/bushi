#include <inttypes.h>
#include <sqlite3.h>
#include <stdio.h>
#include <unistd.h>

#define USE_THE_REPOSITORY_VARIABLE
#include "git-compat-util.h"

#include "commit.h"
#include "config.h"
#include "diff.h"
#include "diffcore.h"
#include "hex.h"
#include "object.h"
#include "path.h"
#include "refs.h"
#include "repository.h"
#include "setup.h"
#include "strbuf.h"
#include "strmap.h"
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

	STMT_UPSERT_REF,

	STMT_UPDATE_REFS_DIRTY,
	STMT_DELETE_DIRTY_REFS,

	STMT_BACKFILL_LIST_FILES,
	STMT_BACKFILL_FILE_COMMITS,
	STMT_BACKFILL_UPDATE_CHANGE,
	STMT_BACKFILL_LOAD_COMMITS,
	STMT_UPDATE_FIRST_DEPTH,

	STMT_STATUS_COMMIT_COUNT,
	STMT_STATUS_FILE_COUNT,
	STMT_STATUS_REF_COUNTS,

	// keep COUNT the last
	STMT_COUNT
};

// clang-format off
const char *texts[STMT_COUNT] = {
	[STMT_INSERT_REPOSITORY] = SQL(
		INSERT INTO repositories
		(      repository_name
		     , repository_path
		     , repository_head
		)
		VALUES
		    (?1, ?2, ?3);
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
	[STMT_BACKFILL_LIST_FILES] = SQL(
		SELECT DISTINCT cg.file_id
		  FROM changes AS cg
		  JOIN commits AS c
		    ON c.commit_id = cg.commit_id
		 WHERE c.repository_id = ?1
		   AND cg.last_commit_id IS NULL;
	),
	[STMT_BACKFILL_FILE_COMMITS] = SQL(
		SELECT cg.commit_id
		     , cg.last_commit_id IS NULL AS need_update
		  FROM changes AS cg
		  JOIN commits AS c
		    ON c.commit_id = cg.commit_id
		 WHERE cg.file_id = ?1
		   AND c.repository_id = ?2;
	),
	[STMT_BACKFILL_UPDATE_CHANGE] = SQL(
		UPDATE changes
		   SET last_commit_id = ?1
		 WHERE commit_id = ?2
		   AND file_id = ?3;
	),
	[STMT_BACKFILL_LOAD_COMMITS] = SQL(
		SELECT c.commit_id
		     , p.commit_id AS parent_id
		  FROM commits AS c
		  LEFT JOIN commits AS p
		    ON c.repository_id = p.repository_id
		   AND c.parent_hash = p.commit_hash
		 WHERE c.repository_id = ?1
		 ORDER BY c.commit_id;
	),
	[STMT_UPDATE_FIRST_DEPTH] = SQL(
		UPDATE commits
		   SET first_depth = ?1
		 WHERE commit_id = ?2;
	),
	[STMT_STATUS_COMMIT_COUNT] = SQL(
		SELECT COUNT(*)
		  FROM commits
		 WHERE repository_id = ?1;
	),
	[STMT_STATUS_FILE_COUNT] = SQL(
		SELECT COUNT(DISTINCT cg.file_id)
		  FROM changes AS cg
		  JOIN commits AS c
		    ON c.commit_id = cg.commit_id
		 WHERE c.repository_id = ?1;
	),
	[STMT_STATUS_REF_COUNTS] = SQL(
		SELECT ref_type
		     , COUNT(*)
		  FROM refs
		 WHERE repository_id = ?1
		 GROUP BY ref_type;
	),
};
// clang-format on

static sqlite3 *conn = NULL;
static sqlite3_stmt *stmts[STMT_COUNT];

static struct strmap file_map;

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
	// generally speaking, it would only be used once.
	sqlite3_reset(stmt);

	int rc;
	while ((rc = sqlite3_step(stmt)) == SQLITE_ROW) {
		const char *name = (const char *)sqlite3_column_text(stmt, 0);
		const char *path = (const char *)sqlite3_column_text(stmt, 1);

		printf("%-32.32s %s\n", name, path);
	}

	if (rc != SQLITE_DONE)
		err("failed to list repositories: %s", sqlite3_errmsg(conn));
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

static bool
branch_exists(struct ref_store *refs, const char *name)
{
	struct strbuf full = STRBUF_INIT;
	bool exists;

	if (starts_with(name, "refs/heads/"))
		strbuf_addstr(&full, name);
	else
		strbuf_addf(&full, "refs/heads/%s", name);
	exists = refs_ref_exists(refs, full.buf);
	strbuf_release(&full);
	return exists;
}

static char *
determine_repository_head(void)
{
	char *head = NULL;
	struct strbuf head_ref = STRBUF_INIT;
	struct ref_store *refs = get_main_ref_store(the_repository);
	const char *fallbacks[] = {
	    "main",
	    "master",
	    "dev",
	    NULL,
	};

	head = value_from_config("bushi.head");
	if (head && *head && branch_exists(refs, head)) {
		dbg("repository head from config: %s", head);
		goto out;
	}
	free(head);
	head = NULL;

	if (!refs_read_symbolic_ref(refs, "HEAD", &head_ref) &&
	    branch_exists(refs, head_ref.buf)) {
		head = xstrdup(head_ref.buf + strlen("refs/heads/"));
		dbg("repository head from HEAD: %s", head);
		goto out;
	}

	for (size_t i = 0; fallbacks[i]; i++) {
		if (branch_exists(refs, fallbacks[i])) {
			head = xstrdup(fallbacks[i]);
			dbg("repository head from fallback: %s", head);
			goto out;
		}
	}

out:
	strbuf_release(&head_ref);
	return head;
}

static char *
determine_repository_name(const char *path)
{
	char *name = NULL;

	name = value_from_config("bushi.name");
	if (name && *name) {
		dbg("repository name from config: %s", name);
		return name;
	}
	free(name);

	name = name_from_path(path);
	dbg("repository name from path: %s", name);
	return name;
}

void
run_add(const char *path)
{
	char *gitdir = NULL;
	char *name = NULL;
	char *head = NULL;

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

	head = determine_repository_head();
	if (!head) {
		err("cannot determine repository head for: %s", path);
		goto out;
	}

	name = determine_repository_name(path);
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
	sqlite3_bind_text(stmt, 3, head, -1, SQLITE_STATIC);

	rc = sqlite3_step(stmt);
	if (rc != SQLITE_DONE)
		err("failed to add repository: %s", sqlite3_errmsg(conn));

out:
	free(head);
	free(name);
	free(gitdir);
}

static int64_t
get_commit_id(int64_t repository_id, const char *hash)
{
	sqlite3_stmt *stmt = stmts[STMT_GET_COMMIT_ID];
	sqlite3_reset(stmt);
	sqlite3_bind_int64(stmt, 1, repository_id);
	sqlite3_bind_text(stmt, 2, hash, -1, SQLITE_STATIC);

	int64_t commit_id = 0;
	int rc = sqlite3_step(stmt);
	if (rc == SQLITE_ROW)
		commit_id = sqlite3_column_int64(stmt, 0);
	return commit_id;
}

static bool
commit_exists(int64_t repository_id, const char *hash)
{
	return get_commit_id(repository_id, hash) != 0;
}

static void
insert_commit(int64_t repository_id, const char *hash, const char *parent_hash)
{
	sqlite3_stmt *stmt = stmts[STMT_INSERT_COMMIT];
	sqlite3_reset(stmt);
	sqlite3_bind_text(stmt, 1, hash, -1, SQLITE_STATIC);
	sqlite3_bind_text(stmt, 2, parent_hash, -1, SQLITE_STATIC);
	sqlite3_bind_int64(stmt, 3, repository_id);

	int rc = sqlite3_step(stmt);
	if (rc != SQLITE_DONE)
		err("failed to insert commit %s: %s", hash,
		    sqlite3_errmsg(conn));
}

static int64_t
get_or_insert_file_id(const char *path)
{
	// Fast in-memory lookup for file_id.
	int64_t file_id = (intptr_t)strmap_get(&file_map, path);
	if (file_id)
		return file_id;

	// Cache miss: try the database first.
	sqlite3_stmt *get_file = stmts[STMT_GET_FILE_ID];
	sqlite3_reset(get_file);
	sqlite3_bind_text(get_file, 1, path, -1, SQLITE_STATIC);
	int rc = sqlite3_step(get_file);
	if (rc == SQLITE_ROW) {
		file_id = sqlite3_column_int64(get_file, 0);
		goto cache;
	}

	// Not in DB either: insert a new file.
	sqlite3_stmt *insert_file = stmts[STMT_INSERT_FILE];
	sqlite3_reset(insert_file);
	sqlite3_bind_text(insert_file, 1, path, -1, SQLITE_STATIC);
	rc = sqlite3_step(insert_file);
	if (rc != SQLITE_DONE) {
		err("failed to insert file %s: %s", path, sqlite3_errmsg(conn));
		return 0;
	}
	file_id = sqlite3_last_insert_rowid(conn);

cache:
	strmap_put(&file_map, path, (void *)(intptr_t)file_id);
	return file_id;
}

static void
insert_changes_for_commit(int64_t repository_id, struct commit *commit)
{
	struct diff_options opt;

	repo_diff_setup(the_repository, &opt);
	opt.flags.recursive = 1;
	opt.detect_rename = 0;
	opt.output_format = DIFF_FORMAT_NO_OUTPUT;
	diff_setup_done(&opt);

	repo_parse_commit(the_repository, commit);

	// Only diff against the first parent.
	if (commit->parents)
		diff_tree_oid(&commit->parents->item->object.oid,
			      &commit->object.oid, "", &opt);
	else
		diff_root_tree_oid(&commit->object.oid, "", &opt);

	diffcore_std(&opt);

	sqlite3_stmt *insert_change = stmts[STMT_INSERT_CHANGE];

	int64_t commit_id =
	    get_commit_id(repository_id, oid_to_hex(&commit->object.oid));
	if (!commit_id)
		goto cleanup;

	for (int i = 0; i < diff_queued_diff.nr; i++) {
		struct diff_filepair *p = diff_queued_diff.queue[i];
		const char *path = p->two->path ? p->two->path : p->one->path;

		int64_t file_id = get_or_insert_file_id(path);
		if (!file_id)
			continue;

		sqlite3_reset(insert_change);
		sqlite3_bind_int64(insert_change, 1, commit_id);
		sqlite3_bind_int64(insert_change, 2, file_id);
		int rc = sqlite3_step(insert_change);
		if (rc != SQLITE_DONE)
			err("failed to insert change: %s",
			    sqlite3_errmsg(conn));
	}

cleanup:
	diff_flush(&opt);
}

static void
walk_commit_history(int64_t repository_id, struct commit *commit)
{
	struct commit_list *stack = NULL;
	commit_list_insert(commit, &stack);

	while (stack) {
		struct commit *c = pop_commit(&stack);
		const char *hash = oid_to_hex(&c->object.oid);

		// If this commit is already indexed, skip it and its ancestors.
		if (commit_exists(repository_id, hash))
			continue;

		repo_parse_commit(the_repository, c);

		// Record only the first parent in the commits table.
		const char *parent_hash = NULL;
		if (c->parents)
			parent_hash = oid_to_hex(&c->parents->item->object.oid);

		insert_commit(repository_id, hash, parent_hash);
		insert_changes_for_commit(repository_id, c);

		// Walk up through *all* parents.
		for (struct commit_list *p = c->parents; p; p = p->next)
			commit_list_insert(p->item, &stack);
	}
}

static int
walk_ref_commits(const struct reference *ref, void *cb_data)
{
	int64_t repository_id = *(int64_t *)cb_data;

	// Resolve ref to a commit; skip anything that is not a commit.
	struct commit *commit =
	    lookup_commit_reference_gently(the_repository, ref->oid, 1);
	if (!commit)
		return 0;

	walk_commit_history(repository_id, commit);
	return 0;
}

static int
insert_ref(const struct reference *ref, void *cb_data)
{
	int64_t repository_id = *(int64_t *)cb_data;

	struct commit *commit =
	    lookup_commit_reference_gently(the_repository, ref->oid, 1);
	if (!commit)
		return 0;

	int64_t commit_id =
	    get_commit_id(repository_id, oid_to_hex(&commit->object.oid));
	if (!commit_id) {
		err("ref %s points to unknown commit %s", ref->name,
		    oid_to_hex(&commit->object.oid));
		return 0;
	}

	const char *show_name = ref->name;
	int ref_type = 0;
	if (starts_with(ref->name, "refs/heads/")) {
		show_name = ref->name + strlen("refs/heads/");
		ref_type = 0;
	} else if (starts_with(ref->name, "refs/tags/")) {
		show_name = ref->name + strlen("refs/tags/");
		ref_type = 1;
	} else {
		return 0; // skip refs/notes, refs/remotes, etc.
	}

	// ref_time: use commit timestamp for MVP
	sqlite3_stmt *stmt = stmts[STMT_UPSERT_REF];
	sqlite3_reset(stmt);
	sqlite3_bind_text(stmt, 1, ref->name, -1, SQLITE_STATIC);
	sqlite3_bind_text(stmt, 2, show_name, -1, SQLITE_STATIC);
	sqlite3_bind_int64(stmt, 3, commit_id);
	sqlite3_bind_int64(stmt, 4, 0); // ref_time placeholder
	sqlite3_bind_int64(stmt, 5, ref_type);
	sqlite3_bind_int64(stmt, 6, repository_id);

	int rc = sqlite3_step(stmt);
	if (rc != SQLITE_DONE)
		err("failed to insert ref %s: %s", ref->name,
		    sqlite3_errmsg(conn));

	return 0;
}

struct idmap {
	int64_t *keys;	// 0 means empty slot
	uint32_t *vals; // local_idx, UINT32_MAX = not found
	size_t cap;
	size_t size;
};

static void
idmap_init(struct idmap *m, size_t num_commits)
{
	m->cap = 32;
	while (m->cap < num_commits * 1.5)
		m->cap *= 2;
	CALLOC_ARRAY(m->keys, m->cap);
	CALLOC_ARRAY(m->vals, m->cap);
	m->size = 0;
}

static void
idmap_clear(struct idmap *m)
{
	free(m->keys);
	free(m->vals);
	m->keys = NULL;
	m->vals = NULL;
	m->cap = 0;
	m->size = 0;
}

static size_t
idmap_slot(const struct idmap *m, int64_t key)
{
	size_t i = (size_t)(key & (int64_t)(m->cap - 1));
	while (m->keys[i] != 0 && m->keys[i] != key) {
		i++;
		if (i == m->cap)
			i = 0;
	}
	return i;
}

static void
idmap_put(struct idmap *m, int64_t key, uint32_t val)
{
	size_t i = idmap_slot(m, key);
	if (m->keys[i] == 0) {
		m->size++;
		m->keys[i] = key;
	}
	m->vals[i] = val;
}

// use UINT32_MAX to indicate "not found".
static uint32_t
idmap_get(const struct idmap *m, int64_t key)
{
	size_t i = idmap_slot(m, key);
	return m->keys[i] == key ? m->vals[i] : UINT32_MAX;
}

struct backfill_index {
	uint32_t num_commits;

	struct idmap idmap; // global commit_id -> commit local_idx

	// input commit local_idx
	int64_t *commit_ids;	// -> global commit_id
	uint32_t *parent_local; // -> parent local_idx (UINT32_MAX = none)
	uint32_t *first_depth;	// -> first-parent depth (UINT32_MAX = unknown)
};

static void
backfill_index_free(struct backfill_index *idx)
{
	if (!idx)
		return;
	free(idx->commit_ids);
	free(idx->parent_local);
	free(idx->first_depth);
	idmap_clear(&idx->idmap);
	free(idx);
}

struct local_index_stack {
	uint32_t *items;
	size_t count;
	size_t alloc;
};

static void
update_first_depth(int64_t commit_id, uint32_t depth)
{
	sqlite3_stmt *stmt = stmts[STMT_UPDATE_FIRST_DEPTH];
	sqlite3_reset(stmt);
	sqlite3_bind_int64(stmt, 1, depth);
	sqlite3_bind_int64(stmt, 2, commit_id);

	int rc = sqlite3_step(stmt);
	if (rc != SQLITE_DONE)
		err("failed to update first_depth: %s", sqlite3_errmsg(conn));
}

static void
backfill_first_depths(struct backfill_index *idx)
{
	struct local_index_stack trail = {0};

	for (uint32_t i = 0; i < idx->num_commits; i++) {
		uint32_t curr = i;
		uint32_t depth;

		if (idx->first_depth[i] != UINT32_MAX)
			continue;

		trail.count = 0;

		while (curr != UINT32_MAX &&
		       idx->first_depth[curr] == UINT32_MAX) {
			ALLOC_GROW(trail.items, trail.count + 1, trail.alloc);
			trail.items[trail.count++] = curr;
			curr = idx->parent_local[curr];
		}

		if (curr == UINT32_MAX)
			depth = 0;
		else
			depth = idx->first_depth[curr] + 1;

		while (trail.count) {
			uint32_t v = trail.items[--trail.count];
			idx->first_depth[v] = depth;
			update_first_depth(idx->commit_ids[v], depth);
			depth++;
		}
	}

	free(trail.items);
}

static struct backfill_index *
build_backfill_index(int64_t repository_id)
{
	struct backfill_index *idx = xcalloc(1, sizeof(*idx));
	struct backfill_index *result = NULL;

	sqlite3_stmt *stmt = stmts[STMT_BACKFILL_LOAD_COMMITS];
	sqlite3_reset(stmt);
	sqlite3_bind_int64(stmt, 1, repository_id);

	int64_t *commit_ids = NULL;
	int64_t *parent_ids = NULL;
	size_t commit_ids_alloc = 0;
	size_t parent_ids_alloc = 0;
	uint32_t num = 0; // local index, starts at 0

	while (sqlite3_step(stmt) == SQLITE_ROW) {
		ALLOC_GROW(commit_ids, num + 1, commit_ids_alloc);
		ALLOC_GROW(parent_ids, num + 1, parent_ids_alloc);

		commit_ids[num] = sqlite3_column_int64(stmt, 0);
		if (sqlite3_column_type(stmt, 1) == SQLITE_NULL)
			parent_ids[num] = 0;
		else
			parent_ids[num] = sqlite3_column_int64(stmt, 1);
		num++;

		if (num == UINT32_MAX) {
			err("backfill index reached UINT32_MAX commits");
			break;
		}
	}

	REALLOC_ARRAY(commit_ids, num);
	idx->commit_ids = commit_ids;
	idx->num_commits = num;

	if (num == 0 || num == UINT32_MAX)
		goto cleanup;

	idmap_init(&idx->idmap, num);
	for (uint32_t i = 0; i < num; i++)
		idmap_put(&idx->idmap, idx->commit_ids[i], i);

	CALLOC_ARRAY(idx->parent_local, num);
	CALLOC_ARRAY(idx->first_depth, num);
	for (uint32_t i = 0; i < num; i++) {
		idx->parent_local[i] = UINT32_MAX;
		idx->first_depth[i] = UINT32_MAX;
		int64_t parent_id = parent_ids[i];
		if (!parent_id)
			continue;

		uint32_t parent_local = idmap_get(&idx->idmap, parent_id);
		if (parent_local == UINT32_MAX) {
			err("parent %" PRId64 " not found in backfill index",
			    parent_id);
			goto cleanup;
		}
		idx->parent_local[i] = parent_local;
	}

	result = idx;

cleanup:
	free(parent_ids);
	if (!result)
		backfill_index_free(idx);
	return result;
}

struct backfill_buf {
	uint8_t *bitmap;
	size_t bitmap_size;
	uint32_t *pending;
	size_t pending_cap;
};

static void
update_last_commit_id(int64_t file_id, int64_t commit_id,
		      int64_t last_commit_id)
{
	sqlite3_stmt *stmt = stmts[STMT_BACKFILL_UPDATE_CHANGE];
	sqlite3_reset(stmt);
	sqlite3_bind_int64(stmt, 1, last_commit_id);
	sqlite3_bind_int64(stmt, 2, commit_id);
	sqlite3_bind_int64(stmt, 3, file_id);

	int rc = sqlite3_step(stmt);
	if (rc != SQLITE_DONE)
		err("failed to update last_commit_id: %s",
		    sqlite3_errmsg(conn));
}

static void
backfill_one_file(int64_t file_id, int64_t repository_id,
		  const struct backfill_index *idx, struct backfill_buf *buf)
{
	sqlite3_stmt *get_commits = stmts[STMT_BACKFILL_FILE_COMMITS];

	memset(buf->bitmap, 0, buf->bitmap_size);
	size_t pending_count = 0;

	// Build bitmap of all commits that touched this file in this
	// repository, and collect pending commit_ids that need updating.
	sqlite3_reset(get_commits);
	sqlite3_bind_int64(get_commits, 1, file_id);
	sqlite3_bind_int64(get_commits, 2, repository_id);
	while (sqlite3_step(get_commits) == SQLITE_ROW) {
		int64_t commit_id = sqlite3_column_int64(get_commits, 0);
		int need_update = sqlite3_column_int(get_commits, 1);

		uint32_t commit_local = idmap_get(&idx->idmap, commit_id);
		if (commit_local == UINT32_MAX) {
			err("commit %" PRId64 " not found in backfill index",
			    commit_id);
			continue;
		}

		buf->bitmap[commit_local / 8] |= 1u << (commit_local % 8);

		if (need_update) {
			ALLOC_GROW(buf->pending, pending_count + 1,
				   buf->pending_cap);
			buf->pending[pending_count++] = commit_local;
		}
	}

	// For each pending commit, walk first-parent chain.
	for (size_t i = 0; i < pending_count; i++) {
		uint32_t curr = buf->pending[i];
		uint32_t last = curr;

		uint32_t ancestor = idx->parent_local[curr];
		while (ancestor != UINT32_MAX) {
			if (buf->bitmap[ancestor / 8] &
			    (1u << (ancestor % 8))) {
				last = ancestor;
				break;
			}
			ancestor = idx->parent_local[ancestor];
		}

		update_last_commit_id(file_id, idx->commit_ids[curr],
				      idx->commit_ids[last]);
	}
}

static void
backfill_repository(int64_t repository_id)
{
	dbg("backfilling repository %" PRId64, repository_id);

	struct backfill_index *idx = build_backfill_index(repository_id);
	if (!idx)
		return;

	backfill_first_depths(idx);

	// List files with at least one unfilled change in this repository.
	sqlite3_stmt *list_files = stmts[STMT_BACKFILL_LIST_FILES];
	sqlite3_reset(list_files);
	sqlite3_bind_int64(list_files, 1, repository_id);

	size_t bitmap_size = (idx->num_commits + 7) / 8;
	struct backfill_buf buf = {
	    .bitmap = NULL,
	    .bitmap_size = bitmap_size,
	    .pending = NULL,
	    .pending_cap = 0,
	};
	CALLOC_ARRAY(buf.bitmap, bitmap_size);

	while (sqlite3_step(list_files) == SQLITE_ROW) {
		int64_t file_id = sqlite3_column_int64(list_files, 0);
		backfill_one_file(file_id, repository_id, idx, &buf);
	}

	free(buf.pending);
	free(buf.bitmap);
	backfill_index_free(idx);

	dbg("backfill done for repository %" PRId64, repository_id);
}

void
run_sync(const char *name)
{
	sqlite3_stmt *stmt = stmts[STMT_GET_REPOSITORY_BY_NAME];
	sqlite3_reset(stmt);
	sqlite3_bind_text(stmt, 1, name, -1, SQLITE_STATIC);

	int rc = sqlite3_step(stmt);
	if (rc != SQLITE_ROW) {
		err("repository not found: %s", name);
		return;
	}

	int64_t repository_id = sqlite3_column_int64(stmt, 0);
	char *gitdir = xstrdup((const char *)sqlite3_column_text(stmt, 1));

	if (repo_init(the_repository, gitdir, NULL) < 0) {
		err("cannot initialize repository: %s", gitdir);
		free(gitdir);
		return;
	}

	// Do not cache the raw commit object buffers; we only need parsed
	// metadata.
	save_commit_buffer = 0;

	// Reduce Git's internal caches; we stream objects and don't need big
	// caches.
	the_repository->settings.delta_base_cache_limit = 0;

	// Initialize on-demand cache for file lookups.
	strmap_init(&file_map);

	dbg("syncing repository %" PRId64 ": %s", repository_id, gitdir);

	db_begin_transaction();

	// Mark all existing refs for this repository as dirty
	stmt = stmts[STMT_UPDATE_REFS_DIRTY];
	sqlite3_reset(stmt);
	sqlite3_bind_int64(stmt, 1, repository_id);
	rc = sqlite3_step(stmt);
	if (rc != SQLITE_DONE)
		err("failed to mark refs dirty: %s", sqlite3_errmsg(conn));

	// Walk each ref's history, inserting commits and changes as we go
	refs_for_each_ref(get_main_ref_store(the_repository), walk_ref_commits,
			  &repository_id);

	// Upsert all current refs; this also clears is_dirty for each live ref
	refs_for_each_ref(get_main_ref_store(the_repository), insert_ref,
			  &repository_id);

	// Delete refs that are no longer present
	stmt = stmts[STMT_DELETE_DIRTY_REFS];
	sqlite3_reset(stmt);
	sqlite3_bind_int64(stmt, 1, repository_id);
	rc = sqlite3_step(stmt);
	if (rc != SQLITE_DONE)
		err("failed to delete dirty refs: %s", sqlite3_errmsg(conn));

	db_end_transaction();

	strmap_clear(&file_map, 0);
	free(gitdir);
	repo_clear(the_repository);

	db_begin_transaction();
	backfill_repository(repository_id);
	db_end_transaction();
}

void
run_status(const char *name)
{
	sqlite3_stmt *stmt = stmts[STMT_GET_REPOSITORY_BY_NAME];
	sqlite3_reset(stmt);
	sqlite3_bind_text(stmt, 1, name, -1, SQLITE_STATIC);

	int rc = sqlite3_step(stmt);
	if (rc != SQLITE_ROW) {
		err("repository not found: %s", name);
		return;
	}

	int64_t repository_id = sqlite3_column_int64(stmt, 0);
	const char *path = (const char *)sqlite3_column_text(stmt, 1);

	int64_t commits = 0;
	int64_t files = 0;
	int64_t branches = 0;
	int64_t tags = 0;

	stmt = stmts[STMT_STATUS_COMMIT_COUNT];
	sqlite3_reset(stmt);
	sqlite3_bind_int64(stmt, 1, repository_id);
	if (sqlite3_step(stmt) == SQLITE_ROW)
		commits = sqlite3_column_int64(stmt, 0);

	stmt = stmts[STMT_STATUS_FILE_COUNT];
	sqlite3_reset(stmt);
	sqlite3_bind_int64(stmt, 1, repository_id);
	if (sqlite3_step(stmt) == SQLITE_ROW)
		files = sqlite3_column_int64(stmt, 0);

	stmt = stmts[STMT_STATUS_REF_COUNTS];
	sqlite3_reset(stmt);
	sqlite3_bind_int64(stmt, 1, repository_id);
	while (sqlite3_step(stmt) == SQLITE_ROW) {
		int ref_type = sqlite3_column_int(stmt, 0);
		int64_t count = sqlite3_column_int64(stmt, 1);
		switch (ref_type) {
		case 0:
			branches = count;
			break;
		case 1:
			tags = count;
			break;
		default:
			err("type not implemented yet");
		}
	}

	printf("repository: %s\n", name);
	printf("path:       %s\n", path);
	printf("commits:    %" PRId64 "\n", commits);
	printf("files:      %" PRId64 "\n", files);
	printf("references: %" PRId64 " ", branches + tags);
	printf("(branches: %" PRId64 ", tags: %" PRId64 ")\n", branches, tags);
}

int
main(int argc, char *const argv[])
{
	const char *path = NULL;
	const char *name = NULL;
	const char *database = NULL;
	int i = 0;
	enum Mode mode = MODE_SYNC;

	while ((i = getopt(argc, argv, "a:t:cfsrldhv")) != -1) {
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
	case MODE_STATUS:
		run_status(name);
		break;
	case MODE_SYNC:
		run_sync(name);
		break;
	default:
		err("mode not implemented yet");
		break;
	}

	db_close();
	return 0;
}
