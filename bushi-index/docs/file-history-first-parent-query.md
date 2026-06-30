# File History First-Parent Query

This design depends on both of these being implemented first:

```text
docs/backfill-first-parent-depth.md
docs/first-parent-ancestor-skip-list.md
```

It also depends on `changes.last_commit_id` being backfilled for each file
change. This document only covers a single `file_id`; directory queries are out
of scope.

## Goal

Optimize this query shape:

```text
Starting from commit B, return recent commits on B's first-parent chain that
modified file F.
```

The target behavior matches a single-file, first-parent-only history query:

```text
git log --first-parent -- <file>
```

The common case is fetching a small page, such as `LIMIT 20`.

## Existing Chain

`changes.last_commit_id` forms a per-file chain between commits that modified
the same file.

For a file `F`, if these commits modified it on the first-parent line:

```text
A <- C <- G <- K
```

Then the `changes` rows should contain:

```text
K.last_commit_id = G
G.last_commit_id = C
C.last_commit_id = A
A.last_commit_id = A
```

The terminal condition is equality:

```text
last_commit_id == commit_id
```

The terminal condition is not `NULL`.

## Problem

Once the first matching file change is known, collecting the next results is
cheap: follow `changes.last_commit_id` until the page limit is reached.

The expensive part is finding that first matching file change when the input
commit did not modify the file.

Without the depth and ancestor skip-list indexes, the query has to walk upward
through first parents one commit at a time until it finds a commit that touched
the file.

## High-Level Strategy

Split the query into two phases:

1. Find the nearest commit at or before the input commit that modified the file.
2. Follow the `changes.last_commit_id` chain from that commit to collect the
   remaining results.

The first phase uses:

```text
commits.first_parent_depth
ancestors(commit_id, exponent, ancestor_id)
changes(file_id, commit_id, last_commit_id)
```

The second phase uses only the `changes.last_commit_id` chain.

## Phase 1: Find The Starting Commit

Given:

```text
repository_id
file_id
input_commit_id
input_depth = first_parent_depth(input_commit_id)
```

Scan commits that modified the file and are not deeper than the input commit:

```sql
SELECT cg.commit_id
     , c.first_parent_depth
  FROM changes AS cg
  JOIN commits AS c
    ON c.commit_id = cg.commit_id
 WHERE cg.file_id = ?1
   AND c.repository_id = ?2
   AND c.first_parent_depth <= ?3
 ORDER BY c.first_parent_depth DESC;
```

For each candidate, test whether it is on the input commit's first-parent chain:

```text
is_first_parent_ancestor(candidate.commit_id, input_commit_id)
```

The first candidate that passes is the starting commit.

Pseudocode:

```text
start = none

for candidate in file_changes_ordered_by_depth_desc(file_id, input_depth):
    if is_first_parent_ancestor(candidate.commit_id, input_commit_id):
        start = candidate.commit_id
        break
```

If no candidate passes, the file has no matching history reachable from the
input commit's first-parent chain.

## Ancestor Check

Use the skip-list from `docs/first-parent-ancestor-skip-list.md`.

Given candidate `A` and input commit `B`:

```text
da = first_parent_depth(A)
db = first_parent_depth(B)
```

If `da > db`, `A` cannot be an ancestor of `B`.

Otherwise, jump `B` upward by `db - da` steps using `ancestors`.

Pseudocode:

```text
is_first_parent_ancestor(a, b):
    da = depth(a)
    db = depth(b)

    if da > db:
        return false

    current = b
    delta = db - da
    exponent = 0

    while delta != 0:
        if delta has lowest bit set:
            current = ancestor(current, exponent)
        delta = delta >> 1
        exponent++

    return current == a
```

When `a == b`, `delta == 0`, so the check returns true. This means if the input
commit itself modified the file, it is returned as the first result.

## Phase 2: Follow The File Change Chain

After finding `start`, recursively follow `changes.last_commit_id` for the same
`file_id`.

The recursion stops when either:

```text
result count reaches limit
last_commit_id == commit_id
```

Pseudocode SQL shape:

```sql
WITH RECURSIVE history(commit_id, depth) AS (
    SELECT ?1 AS commit_id
         , 1  AS depth

    UNION ALL

    SELECT cg.last_commit_id
         , h.depth + 1
      FROM history AS h
      JOIN changes AS cg
        ON cg.commit_id = h.commit_id
       AND cg.file_id = ?2
     WHERE cg.last_commit_id != h.commit_id
       AND h.depth < ?3
)
SELECT c.commit_id
     , c.commit_hash
  FROM history AS h
  JOIN commits AS c
    ON c.commit_id = h.commit_id
 ORDER BY h.depth;
```

Parameters:

```text
?1 = start_commit_id
?2 = file_id
?3 = limit
```

## Full Query Flow

```text
file_history_first_parent(repository_id, file_id, input_commit_id, limit):
    input_depth = first_parent_depth(input_commit_id)

    start = find_latest_file_change_ancestor(
        repository_id,
        file_id,
        input_commit_id,
        input_depth
    )

    if start is none:
        return empty result

    return collect_history_by_last_commit_id(file_id, start, limit)
```

## Useful Indexes

Find candidate file changes:

```sql
CREATE INDEX IF NOT EXISTS idx_changes_file_commit
    ON changes (
       file_id
     , commit_id
       );
```

Read commit depths while scanning candidates:

```sql
CREATE INDEX IF NOT EXISTS idx_commits_depth
    ON commits (
       repository_id
     , first_parent_depth
     , commit_id
       );
```

Follow the `last_commit_id` chain by `(commit_id, file_id)` using the existing
primary key on `changes`:

```text
PRIMARY KEY (commit_id, file_id)
```

## Complexity

The starting commit search scans only commits that modified the target file and
whose depth is no greater than the input commit's depth.

Each candidate uses an ancestor check with at most:

```text
O(log depth_gap)
```

skip-list lookups.

After the starting commit is found, collecting a page of size `L` follows at
most `L` rows in the `changes.last_commit_id` chain.
