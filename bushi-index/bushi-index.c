#define _GNU_SOURCE

#include <assert.h>
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

#define info(FMT, ...)                                                         \
  fprintf(stdout, "%5d | %-16.16s " FMT "\n", __LINE__,                        \
          __func__ __VA_OPT__(, ) __VA_ARGS__)

#define SQL(...) #__VA_ARGS__

#ifndef RUN_TEST
#define sync_main main
#endif

enum {
  STMT_UPSERT_REPOSITORY,
  STMT_GET_REPOSITORY_ID,

  STMT_GET_COMMIT_ID,
  STMT_INSERT_COMMIT,

  STMT_GET_FILE_ID,
  STMT_INSERT_FILE,

  STMT_INSERT_CHANGE,
  STMT_UPDATE_GENERATION,

  // keep COUNT the last
  STMT_COUNT
};

// sqlite3_finalize
static sqlite3_stmt *stmts[STMT_COUNT];
static sqlite3 *connection;
static int64_t repository_id;
static git_repository *repository_git;

static size_t str_with_sfx(const char *str, const char *sfx) {
  if (!str || !sfx) {
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

static const char *name_from_path(const char *path) {
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

static void db_begin_transaction(void) {
  char *errmsg = NULL;
  int rc;

  rc = sqlite3_exec(connection, "BEGIN TRANSACTION;", NULL, NULL, &errmsg);
  if (rc != SQLITE_OK) {
    info("? %s", errmsg);
    sqlite3_free(errmsg);
    abort();
  }
}

static void db_end_transaction(void) {
  char *errmsg = NULL;
  int rc;

  rc = sqlite3_exec(connection, "COMMIT;", NULL, NULL, &errmsg);
  if (rc != SQLITE_OK) {
    info("? %s", errmsg);
    sqlite3_free(errmsg);
    abort();
  }
}

static bool db_prepare(const char *path) {
  sqlite3 *conn = NULL;
  char *errmsg = NULL;
  const char *sql;
  int rc;

  rc = sqlite3_open(path, &conn);
  if (rc != SQLITE_OK) {
    goto err_open;
  }

  sql = SQL(
      -- ?\n
      PRAGMA synchronous = OFF;

      CREATE TABLE IF NOT EXISTS repositories(
          repository_id INTEGER PRIMARY KEY AUTOINCREMENT,
          name TEXT UNIQUE NOT NULL,        // use in URL
          path TEXT UNIQUE NOT NULL,        // GIT_DIR
          head TEXT
      ) STRICT;

      CREATE TABLE IF NOT EXISTS commits(
          commit_id INTEGER PRIMARY KEY AUTOINCREMENT,
          commit_hash TEXT NOT NULL,
          parent_hash TEXT,                 // only first parent
          generation INTEGER,               // NOT NULL after stage2
          repository_id INTEGER NOT NULL
      ) STRICT;

      CREATE INDEX IF NOT EXISTS idx_commit_hash
          ON commits(repository_id, commit_hash);
      CREATE INDEX IF NOT EXISTS idx_parent_hash
          ON commits(repository_id, parent_hash)
          WHERE generation IS NOT NULL;

      CREATE TABLE IF NOT EXISTS ancestors(
          commit_id INTEGER NOT NULL,
          exponent INTEGER NOT NULL,        // 2^n generation
          ancestor_id INTEGER NOT NULL,     // aka. commit_id
          PRIMARY KEY(commit_id, exponent)
      ) WITHOUT ROWID, STRICT;
// /*
      CREATE TRIGGER IF NOT EXISTS tgr_ancestor
      AFTER UPDATE OF generation ON commits
      FOR EACH ROW
      WHEN NEW.parent_hash IS NOT NULL
      BEGIN
          INSERT INTO ancestors(
              commit_id, exponent, ancestor_id
          )
          WITH RECURSIVE skip_list_cte(commit_id, exponent, ancestor_id) AS(
          SELECT
              NEW.commit_id,
              0 AS exponent,
              c.commit_id AS ancestor_id
          FROM
              commits AS c
          WHERE
              repository_id = NEW.repository_id
              AND commit_hash = NEW.parent_hash

          UNION ALL

          SELECT
              s.commit_id,
              s.exponent + 1,
              a.ancestor_id
          FROM
              skip_list_cte AS s
          INNER JOIN
              ancestors AS a
          ON
              a.commit_id = s.ancestor_id
              AND a.exponent = s.exponent
          )

          SELECT
              commit_id, exponent, ancestor_id
          FROM
              skip_list_cte
          WHERE
              ancestor_id IS NOT NULL;
      END;
// */
      CREATE TABLE IF NOT EXISTS files(
          file_id INTEGER PRIMARY KEY AUTOINCREMENT,
          name TEXT UNIQUE NOT NULL         // just like the hashmap
      ) STRICT;

      CREATE TABLE IF NOT EXISTS changes(
          commit_id INTEGER NOT NULL,
          file_id INTEGER NOT NULL,
          PRIMARY KEY(commit_id, file_id)
      ) WITHOUT ROWID, STRICT;

      CREATE TABLE IF NOT EXISTS branches(
          name TEXT NOT NULL,               // e.g. fix/issue-1
          commit_id INTEGER NOT NULL,       // what about commit_hash?
          time INTEGER NOT NULL,            // commit time
          repository_id INTEGER NOT NULL,
          PRIMARY KEY(repository_id, name)
      ) WITHOUT ROWID, STRICT;

      CREATE INDEX IF NOT EXISTS idx_branches_time
          ON branches(time);

      CREATE TABLE IF NOT EXISTS tags(
          name TEXT NOT NULL,
          commit_id INTEGER NOT NULL,       // always commti
          time INTEGER NOT NULL,            // commit time
          repository_id INTEGER NOT NULL,
          PRIMARY KEY(repository_id, name)
      ) WITHOUT ROWID, STRICT;

      CREATE INDEX IF NOT EXISTS idx_tags_time
          ON tags(time);
  );

  rc = sqlite3_exec(conn, sql, NULL, NULL, &errmsg);
  if (rc != SQLITE_OK) {
    goto err_init;
  }

  sql = SQL(
      -- ? \n
      INSERT INTO repositories(
          name, path, head
      )
      VALUES
          (?1, ?2, ?3)
      ON CONFLICT(name)
      DO UPDATE SET
          path = excluded.path,
          head = excluded.head;
  );

  rc = sqlite3_prepare_v2(conn, sql, strlen(sql) + 1,
                          &stmts[STMT_UPSERT_REPOSITORY], NULL);
  if (rc != SQLITE_OK) {
    goto err_stmt;
  }

  sql = SQL(
      -- ? \n
      SELECT repository_id FROM repositories WHERE name = ?1 LIMIT 1;
  );
  rc = sqlite3_prepare_v2(conn, sql, strlen(sql) + 1,
                          &stmts[STMT_GET_REPOSITORY_ID], NULL);
  if (rc != SQLITE_OK) {
    goto err_stmt;
  }

  sql = SQL(
      -- ?\n
      SELECT
          commit_id
      FROM
          commits
      WHERE
          repository_id = ?1
          AND commit_hash = ?2
      LIMIT
          1;
  );
  rc = sqlite3_prepare_v2(conn, sql, strlen(sql) + 1,
                          &stmts[STMT_GET_COMMIT_ID], NULL);
  if (rc != SQLITE_OK) {
    goto err_stmt;
  }

  sql = SQL(
      -- ?\n
      INSERT INTO commits(
          commit_hash,
          parent_hash,
          generation,
          repository_id
      )
      VALUES
          (?1, ?2, ?3, ?4);
  );
  rc = sqlite3_prepare_v2(conn, sql, strlen(sql) + 1,
                          &stmts[STMT_INSERT_COMMIT], NULL);
  if (rc != SQLITE_OK) {
    goto err_stmt;
  }

  sql = SQL(
      -- ?\n
      SELECT file_id FROM files WHERE name = ?1 LIMIT 1;
  );
  rc = sqlite3_prepare_v2(conn, sql, strlen(sql) + 1, &stmts[STMT_GET_FILE_ID],
                          NULL);
  if (rc != SQLITE_OK) {
    goto err_stmt;
  }

  sql = SQL(
      -- ?\n
      INSERT INTO files(name) VALUES (?1);
  );
  rc = sqlite3_prepare_v2(conn, sql, strlen(sql) + 1, &stmts[STMT_INSERT_FILE],
                          NULL);
  if (rc != SQLITE_OK) {
    goto err_stmt;
  }

  sql = SQL(
      -- ?\n
      INSERT INTO changes(commit_id, file_id) VALUES (?1, ?2);
  );
  rc = sqlite3_prepare_v2(conn, sql, strlen(sql) + 1,
                          &stmts[STMT_INSERT_CHANGE], NULL);
  if (rc != SQLITE_OK) {
    goto err_stmt;
  }

  sql = SQL(
      -- ? \n
      UPDATE
          commits
      SET
          generation = parent.generation + 1
      FROM
          commits AS parent
      WHERE
          commits.commit_id = ?1
          AND parent.generation IS NOT NULL
          AND parent.commit_hash = commits.parent_hash
          AND parent.repository_id = commits.repository_id;
  );
  rc = sqlite3_prepare_v2(conn, sql, strlen(sql) + 1,
                          &stmts[STMT_UPDATE_GENERATION], NULL);
  if (rc != SQLITE_OK) {
    goto err_stmt;
  }

  connection = conn;
  return true;

err_stmt:
  info("%s", sqlite3_errmsg(conn));
  sqlite3_close(conn);
  return false;

err_init:
  info("%s", errmsg);
  sqlite3_free(errmsg);
  sqlite3_close(conn);
  return false;

err_open:
  info("%s", sqlite3_errmsg(conn));
  return false;
}

static void db_cleanup(void) {
  sqlite3 *conn = connection;
  for (int i = 0; i < STMT_COUNT; i++) {
    sqlite3_finalize(stmts[i]);
  }
  sqlite3_close(conn);
  connection = NULL;
}

// ROWID is always not zero
// remember to update head after scanning branches
static bool db_sync_repository_id(const char *name, const char *path,
                                  const char *head) {
  assert(name && path);
  sqlite3_stmt *stmt = NULL;
  int64_t id = 0;
  int rc;

  stmt = stmts[STMT_UPSERT_REPOSITORY];
  sqlite3_reset(stmt);
  sqlite3_clear_bindings(stmt);

  sqlite3_bind_text(stmt, 1, name, -1, SQLITE_STATIC);
  sqlite3_bind_text(stmt, 2, path, -1, SQLITE_STATIC);
  sqlite3_bind_text(stmt, 3, head, -1, SQLITE_STATIC);
  info("$ name: %s, path: %s, head: %s", name, path, head ? head : "NULL");
  rc = sqlite3_step(stmt);
  if (rc != SQLITE_DONE) {
    info("? %s", sqlite3_errmsg(connection));
    return false;
  }

  stmt = stmts[STMT_GET_REPOSITORY_ID];
  sqlite3_reset(stmt);
  sqlite3_clear_bindings(stmt);

  sqlite3_bind_text(stmt, 1, name, -1, SQLITE_STATIC);
  info("$ name: %s", name);
  rc = sqlite3_step(stmt);
  if (rc != SQLITE_ROW) {
    info("? %s", sqlite3_errmsg(connection));
    return false;
  }
  id = sqlite3_column_int64(stmt, 0);
  info("* repository_id: %ld", id);
  repository_id = id;
  return true;
}

static void git_cleanup(void) {
  git_repository_free(repository_git);
  git_libgit2_shutdown();
}

static bool git_prepare(const char *git_dir) {
  git_repository *repo_git;
  bool is_ok = true;
  int rc = 0;

  git_libgit2_init();
  git_libgit2_opts(GIT_OPT_ENABLE_CACHING, false);
  git_libgit2_opts(GIT_OPT_ENABLE_STRICT_HASH_VERIFICATION, false);

  info("$ GIT_DIR: %s", git_dir);
  setenv("GIT_DIR", git_dir, 1);

  rc = git_repository_open_bare(&repo_git, git_dir);
  if (rc < 0) {
    const git_error *e = git_error_last();
    info("%s", e->message);
    return false;
  }

  git_config *config;
  rc = git_repository_config_snapshot(&config, repo_git);
  if (rc < 0) {
  }

  // head is the same, but update it after we scan all branches
  const char *name = NULL;
  rc = git_config_get_string(&name, config, "bushi.name");
  if (rc < 0) {
  }

  if (!name || !name[0]) {
    name = name_from_path(git_dir);
  }
  if (!name) {
    return false;
  }
  is_ok = db_sync_repository_id(name, git_dir, NULL);
  if (!is_ok) {
    return false;
  }

  repository_git = repo_git;
  git_config_free(config);
  return true;
}

static int64_t db_get_file_id(const char *name) {
  sqlite3_stmt *stmt;
  int64_t id = 0;
  int rc;

  assert(name);
  info("$ file_name: %s", name);

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

  sqlite3_bind_text(stmt, 1, name, -1, SQLITE_STATIC);
  rc = sqlite3_step(stmt);
  if (rc != SQLITE_DONE) {
    info("? %s", sqlite3_errmsg(connection));
    goto done;
  }
  id = sqlite3_last_insert_rowid(connection);

done:
  info("* file_id: %ld", id);
  return id;
}

static void db_insert_change(int64_t commit_id, int64_t file_id) {
  assert(commit_id);
  assert(file_id);
  info("$ commit_id: %ld, file_id: %ld", commit_id, file_id);

  sqlite3_stmt *stmt = stmts[STMT_INSERT_CHANGE];
  sqlite3_reset(stmt);
  sqlite3_clear_bindings(stmt);

  sqlite3_bind_int64(stmt, 1, commit_id);
  sqlite3_bind_int64(stmt, 2, file_id);

  int rc = sqlite3_step(stmt);
  if (rc != SQLITE_DONE) {
    info("? %s", sqlite3_errmsg(connection));
  }
  return;
}

static int64_t db_get_commit_id(const char *commit_hash) {
  int64_t id = 0;
  int rc;

  assert(repository_id);
  assert(commit_hash);

  sqlite3_stmt *stmt = stmts[STMT_GET_COMMIT_ID];
  sqlite3_reset(stmt);
  sqlite3_clear_bindings(stmt);

  sqlite3_bind_int(stmt, 1, repository_id);
  sqlite3_bind_text(stmt, 2, commit_hash, -1, SQLITE_STATIC);
  info("$ commit_hash: %s", commit_hash);
  rc = sqlite3_step(stmt);
  if (rc != SQLITE_ROW) {
    info("? %s", sqlite3_errmsg(connection));
    goto done;
  }
  id = sqlite3_column_int64(stmt, 0);

done:
  info("* commit_id: %ld", id);
  return id;
}

static int64_t db_insert_commit(const char *commit_hash,
                                const char *parent_hash) {
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
  sqlite3_bind_text(stmt, 1, commit_hash, -1, NULL);
  if (parent_hash) {
    assert(parent_hash[0]);
    sqlite3_bind_text(stmt, 2, parent_hash, -1, NULL);
  } else {
    sqlite3_bind_null(stmt, 2);     // root commit
    sqlite3_bind_int64(stmt, 3, 0); // generation
  }
  sqlite3_bind_int64(stmt, 4, repository_id);

  int rc = sqlite3_step(stmt);
  if (rc != SQLITE_DONE) {
    info("? %s", sqlite3_errmsg(connection));
    goto done;
  }
  id = sqlite3_last_insert_rowid(connection);

done:
  info("* id: %ld", id);
  return id;
}

static bool db_update_generation(int64_t commit_id) {
  assert(commit_id);

  sqlite3_stmt *stmt = stmts[STMT_UPDATE_GENERATION];
  sqlite3_reset(stmt);
  sqlite3_clear_bindings(stmt);

  info("$ commit_id: %ld", commit_id);
  sqlite3_bind_int64(stmt, 1, commit_id);

  int rc = sqlite3_step(stmt);
  if (rc != SQLITE_DONE) {
    info("? %s", sqlite3_errmsg(connection));
    return false;
  }

  int rf = sqlite3_changes(connection);
  if (rf == 0) {
    info("? no changes");
  }
  return true;
}

static const char *commit_hash_from_object(const git_commit *commit) {
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

static void sync_commit_list(git_commit *commit) {
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
      git_commit_parent(&parent, walker, 0); // only first
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
  info("from %s to %s", new_hash, old_hash ? old_hash : "NULL");

  char commit_range[GIT_OID_MAX_HEXSIZE * 2 + 3];
  if (old_hash) {
    snprintf(commit_range, sizeof(commit_range), "%s..%s", old_hash, new_hash);
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
  pipe(pipefd);
  pid_t pid = fork();
  if (pid == 0) {
    close(pipefd[0]);
    dup2(pipefd[1], STDOUT_FILENO);
    close(pipefd[1]);
    execvp("git", args);
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
        break;
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
  waitpid(pid, &status, 0);
}

static int sync_branch_handle(const char *name, void *payload) {
  (void)payload;

  info("$ branch: %s", name);

  git_reference *ref;
  git_reference_lookup(&ref, repository_git, name);

  git_commit *target;
  git_reference_peel((git_object **)&target, ref, GIT_OBJECT_COMMIT);

  sync_commit_list(target);
  return 0;
}

static void print_usage(void) {
  puts("usage: bushi-index -t DATABASE -p GIT_DIR\n"
       "       bushi-index -t DATABASE -d NAME\n");
}

int sync_main(int argc, char **argv) {
  char *repo_path = NULL;
  char *repo_name = NULL;
  bool is_delete = false;
  char *db_path = NULL;
  bool is_ok;
  int rc;
  int opt;

  while ((opt = getopt(argc, argv, "+p:d:t:")) != -1) {
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
    default:
      print_usage();
      return EXIT_FAILURE;
    }
  }

  if (optind != argc || !db_path || (is_delete ? !repo_name : !repo_path)) {
    print_usage();
    return EXIT_FAILURE;
  }

  is_ok = db_prepare(db_path);
  if (!is_ok) {
  }

  is_ok = git_prepare(repo_path);
  if (!is_ok) {
  }

  //
  rc = git_reference_foreach_glob(repository_git, "refs/heads/*",
                                  sync_branch_handle, NULL);
  if (rc < 0) {
  }
  // same tag handle

  //

  git_cleanup();
  db_cleanup();
  return EXIT_SUCCESS;
}
