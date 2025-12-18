#define _GNU_SOURCE

#include <CUnit/Basic.h>
#include <CUnit/CUnit.h>
#include <CUnit/TestDB.h>

#define RUN_TEST

#include "bushi-index.c"

static void test_name_from_path(void) {
  struct test_case {
    const char *path;
    const char *name;
  };

  struct test_case cases[] = {
      {"/path/to/repo.git", "repo"},
      {"/path/to/repo/.git", "repo"},
      {"/path/to/repo", "repo"},
      {"/path/to/user.repo.git", "user.repo"},
      {NULL, NULL},
  };

  for (size_t i = 0; true; i++) {
    if (!cases[i].path) {
      break;
    }
    const char *actual = name_from_path(cases[i].path);
    const char *expected = cases[i].name;
    CU_ASSERT_STRING_EQUAL(actual, expected);
  }

  const char *value = name_from_path("/.git");
  CU_ASSERT_PTR_NULL(value);
}

static int test_db_clean(void) {
  db_cleanup();
  system("rm -r /tmp/bushi-test");
  return 0;
}

static int test_db_init(void) {
  mkdir("/tmp/bushi-test", 0700);
  char db_path[PATH_MAX];
  snprintf(db_path, sizeof(db_path), "/tmp/bushi-test/bushi-index-%ld.db",
           (long)time(NULL));
  bool is_ok = db_prepare(db_path);
  return is_ok ? 0 : 1;
}

static void test_db_prepare(void) {
  CU_ASSERT_PTR_NOT_NULL(connection);
  for (int i = 0; i < STMT_COUNT; i++) {
    CU_ASSERT_PTR_NOT_NULL(stmts[i]);
  }
}

static void test_db_sync_repository_id(void) {
  const char *name = "test-repo";
  const char *path = "/path/to/repo.git";
  const char *head = "master";

  bool is_ok = db_sync_repository_id(name, path, NULL);
  CU_ASSERT_TRUE(is_ok);
  CU_ASSERT_NOT_EQUAL(repository_id, 0);

  int64_t first_id = repository_id;

  is_ok = db_sync_repository_id(name, path, head);
  CU_ASSERT_TRUE(is_ok);
  CU_ASSERT_EQUAL(repository_id, first_id);
}

static void test_db_get_file_id(void) {
  const char *file_name = "src/main.c";
  int64_t file_id1 = db_get_file_id(file_name);
  CU_ASSERT_NOT_EQUAL(file_id1, 0);

  int64_t file_id2 = db_get_file_id(file_name);
  CU_ASSERT_EQUAL(file_id1, file_id2);

  const char *file_name2 = "src/utils.c";
  int64_t file_id3 = db_get_file_id(file_name2);
  CU_ASSERT_NOT_EQUAL(file_id3, 0);
  CU_ASSERT_NOT_EQUAL(file_id1, file_id3);
}

static void test_commit_hash_from_object(void) {
  // TODO we need a true repository to lookup commits
}

int main(void) {
  CU_initialize_registry();

  CU_pSuite utils = CU_add_suite("utils", NULL, NULL);
  CU_ADD_TEST(utils, test_name_from_path);

  CU_pSuite db = CU_add_suite("db", test_db_init, test_db_clean);
  CU_ADD_TEST(db, test_db_prepare);
  CU_ADD_TEST(db, test_db_get_file_id);
  CU_ADD_TEST(db, test_db_sync_repository_id);

  CU_pSuite git = CU_add_suite("git", NULL, NULL);
  CU_ADD_TEST(git, test_commit_hash_from_object);

  CU_basic_run_tests();

  CU_cleanup_registry();
  return EXIT_SUCCESS;
}
