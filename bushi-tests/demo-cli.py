#!/usr/bin/env python3

import argparse
import os
import signal
import sqlite3
import sys


signal.signal(signal.SIGPIPE, signal.SIG_DFL)

USAGE = "usage: demo-cli.py [-t DATABASE] REPO_NAME -- [FIlE_PATH]"


def print_usage(file=sys.stdout):
    print(USAGE, file=file)


def fail(message):
    """Print one error line and exit with status 1."""
    print(message, file=sys.stderr)
    sys.exit(1)


def error_message(exc):
    return str(exc) or exc.__class__.__name__


class SilentArgumentParser(argparse.ArgumentParser):
    """Argument parser that prints one error line."""

    def error(self, message):
        fail(f"error: {message}")


def parse_args(argv):
    if argv is None:
        argv = sys.argv[1:]

    if not argv:
        print_usage(sys.stderr)
        sys.exit(1)

    if "-h" in argv or "--help" in argv:
        print_usage()
        sys.exit(0)

    parser = SilentArgumentParser(
        prog="demo-cli.py",
        description="Query commit history from a bushi-index database.",
        add_help=False,
    )
    parser.add_argument(
        "-t",
        dest="database",
        help="SQLite database path",
    )
    parser.add_argument(
        "repo",
        help="Repository name",
    )
    parser.add_argument(
        "paths",
        nargs="*",
        help="Optional file path filter",
    )
    return parser.parse_args(argv)


def open_database(path):
    """Open the SQLite database read-only."""
    conn = sqlite3.connect(f"file:{path}?mode=ro", uri=True)
    conn.execute("PRAGMA query_only = ON")
    conn.execute("PRAGMA busy_timeout = 5000")
    return conn


def get_repository_id(conn, name):
    row = conn.execute(
        "SELECT repository_id FROM repositories WHERE repository_name = ?",
        (name,),
    ).fetchone()
    if row is None:
        raise ValueError("repository not found")
    return row[0]


def get_repository_head(conn, repository_id):
    row = conn.execute(
        """
        SELECT repository_head
          FROM repositories
         WHERE repository_id = ?
        """,
        (repository_id,),
    ).fetchone()
    if row is None or row[0] is None:
        raise ValueError("repository head not set")
    return row[0]


def get_start_commit_id(conn, repository_id):
    head = get_repository_head(conn, repository_id)
    row = conn.execute(
        """
        SELECT commit_id
          FROM refs
         WHERE repository_id = ?
           AND full_name = ?
        """,
        (repository_id, f"refs/heads/{head}"),
    ).fetchone()
    if row is None:
        raise ValueError("no ref")
    return row[0]


def get_commit_depth(conn, commit_id):
    row = conn.execute(
        "SELECT first_depth FROM commits WHERE commit_id = ?",
        (commit_id,),
    ).fetchone()
    if row is None:
        raise ValueError("commit not found")
    return row[0]


def get_ancestor(conn, commit_id, exponent):
    row = conn.execute(
        "SELECT ancestor_id FROM ancestors WHERE commit_id = ? AND exponent = ?",
        (commit_id, exponent),
    ).fetchone()
    if row is None:
        raise ValueError("ancestor not found")
    return row[0]


def is_first_parent_ancestor(conn, candidate_id, input_id):
    """Return True if candidate is on the first-parent chain of input."""
    if candidate_id == input_id:
        return True

    da = get_commit_depth(conn, candidate_id)
    db = get_commit_depth(conn, input_id)

    if da > db:
        return False

    current = input_id
    delta = db - da
    exponent = 0
    while delta:
        if delta & 1:
            current = get_ancestor(conn, current, exponent)
        delta >>= 1
        exponent += 1

    return current == candidate_id


def query_no_path(conn, start_commit_id):
    """Return all commits on the first-parent chain, newest first."""
    cursor = conn.execute(
        """
        WITH RECURSIVE history(commit_id, seq) AS (
            SELECT ?, 0

            UNION ALL

            SELECT a.ancestor_id,
                   h.seq + 1
              FROM history AS h
              JOIN ancestors AS a
                ON a.commit_id = h.commit_id
               AND a.exponent = 0
        )
        SELECT c.commit_hash
          FROM history AS h
          JOIN commits AS c
            ON c.commit_id = h.commit_id
         ORDER BY h.seq ASC
        """,
        (start_commit_id,),
    )
    return [row[0] for row in cursor]


def find_file_start_commit(conn, repository_id, file_id, input_commit_id):
    """Find the nearest commit on the first-parent chain that modified file_id."""
    input_depth = get_commit_depth(conn, input_commit_id)

    cursor = conn.execute(
        """
        SELECT cg.commit_id
          FROM changes AS cg
          JOIN commits AS c
            ON c.commit_id = cg.commit_id
         WHERE cg.file_id = ?
           AND c.repository_id = ?
           AND c.first_depth <= ?
         ORDER BY c.first_depth DESC
        """,
        (file_id, repository_id, input_depth),
    )

    for row in cursor:
        candidate_id = row[0]
        if is_first_parent_ancestor(conn, candidate_id, input_commit_id):
            return candidate_id

    return None


def query_file_history(conn, file_id, start_commit_id):
    """Return all commits that touched file_id, newest first."""
    cursor = conn.execute(
        """
        WITH RECURSIVE history(commit_id, seq) AS (
            SELECT ? AS commit_id,
                   0  AS seq

            UNION ALL

            SELECT cg.last_commit_id,
                   h.seq + 1
              FROM history AS h
              JOIN changes AS cg
                ON cg.commit_id = h.commit_id
               AND cg.file_id = ?
             WHERE cg.last_commit_id != h.commit_id
        )
        SELECT c.commit_hash
          FROM history AS h
          JOIN commits AS c
            ON c.commit_id = h.commit_id
         ORDER BY h.seq ASC
        """,
        (start_commit_id, file_id),
    )
    return [row[0] for row in cursor]


def main(argv=None):
    args = parse_args(argv)

    path = args.database or os.environ.get("BUSHI_DATABASE")
    if not path:
        fail("database path required")

    try:
        conn = open_database(path)
    except sqlite3.Error as exc:
        fail(error_message(exc))

    try:
        repository_id = get_repository_id(conn, args.repo)
        start_commit_id = get_start_commit_id(conn, repository_id)

        if args.paths:
            if len(args.paths) > 1:
                fail("only one path is supported")
            file_path = args.paths[0]

            row = conn.execute(
                "SELECT file_id FROM files WHERE name = ? LIMIT 1",
                (file_path,),
            ).fetchone()
            if row is None:
                return 0
            file_id = row[0]

            file_start_id = find_file_start_commit(
                conn, repository_id, file_id, start_commit_id
            )
            if file_start_id is None:
                return 0

            results = query_file_history(conn, file_id, file_start_id)
        else:
            results = query_no_path(conn, start_commit_id)
    except (sqlite3.Error, ValueError) as exc:
        fail(error_message(exc))
    finally:
        conn.close()

    for commit_hash in results:
        print(commit_hash)

    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except Exception as exc:
        fail(error_message(exc))
