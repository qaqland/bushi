# First-Parent Ancestor Skip List

This design depends on `docs/backfill-first-parent-depth.md` being implemented
first. The skip-list table assumes that every commit can get a stable
`first_parent_depth` during backfill.

## Goal

Provide a fast way to answer this question:

```text
Is commit A on commit B's first-parent chain?
```

The target query should avoid walking one parent at a time when the depth gap is
large.

## Data Model

Store binary-lifting ancestors in a separate table:

```sql
CREATE TABLE IF NOT EXISTS ancestors
(      commit_id    INTEGER NOT NULL
     , exponent     INTEGER NOT NULL
     , ancestor_id  INTEGER NOT NULL
     , PRIMARY KEY (commit_id, exponent)
) WITHOUT ROWID, STRICT;
```

Each row means:

```text
ancestor_id is commit_id's 2^exponent-th first-parent ancestor.
```

Examples:

```text
(B, 0, A) means A is B's 1-step ancestor.
(D, 1, B) means B is D's 2-step ancestor.
(H, 3, X) means X is H's 8-step ancestor.
```

For a chain:

```text
A(root) <- B <- C <- D <- E
```

Rows for `E` are:

```text
(E, 0, D)  -- 2^0 = 1 step
(E, 1, C)  -- 2^1 = 2 steps
(E, 2, A)  -- 2^2 = 4 steps
```

## Write Principle

When a commit receives `first_parent_depth`, insert all skip-list rows for that
commit.

The first row is its direct first parent:

```text
ancestor(commit, 0) = first_parent(commit)
```

Higher rows are built by doubling:

```text
ancestor(commit, k + 1) = ancestor(ancestor(commit, k), k)
```

This means a commit can build its own skip-list rows if its first parent has
already built skip-list rows.

## Trigger-Based Population

The table can be maintained by an `AFTER UPDATE OF first_parent_depth` trigger on
`commits`.

The trigger should run only for commits with a first parent:

```sql
WHEN NEW.parent_hash IS NOT NULL
```

The trigger inserts rows with a recursive CTE:

```text
base row:
    NEW commit's 2^0 ancestor is its first parent

recursive row:
    if X is NEW commit's 2^k ancestor,
    and Y is X's 2^k ancestor,
    then Y is NEW commit's 2^(k+1) ancestor
```

Pseudocode SQL shape:

```sql
INSERT INTO ancestors (commit_id, exponent, ancestor_id)
WITH RECURSIVE skip_list(commit_id, exponent, ancestor_id) AS (
    SELECT NEW.commit_id,
           0,
           parent.commit_id
      FROM commits AS parent
     WHERE parent.repository_id = NEW.repository_id
       AND parent.commit_hash = NEW.parent_hash

    UNION ALL

    SELECT s.commit_id,
           s.exponent + 1,
           a.ancestor_id
      FROM skip_list AS s
      JOIN ancestors AS a
        ON a.commit_id = s.ancestor_id
       AND a.exponent = s.exponent
)
SELECT commit_id, exponent, ancestor_id
  FROM skip_list;
```

## Backfill Ordering

The depth backfill should update `commits.first_parent_depth` at the same time it
assigns each in-memory depth.

During path unwinding, commits are visited from root direction toward child
direction:

```text
root-side ancestor -> ... -> child
```

So the database update can happen immediately after assigning the in-memory
depth:

```text
fp_depth[v] = depth
UPDATE commits SET first_parent_depth = depth WHERE commit_id = commit_ids[v]
depth++
```

This ordering ensures that when the trigger runs for a child, the child's first
parent has already received `first_parent_depth` and has already populated its
own `ancestors` rows.

## Query Principle

To test whether `A` is on `B`'s first-parent chain:

1. Read `first_parent_depth` for both commits.
2. If `depth(A) > depth(B)`, return false.
3. Move `B` upward by `depth(B) - depth(A)` steps using the skip-list table.
4. Return whether the landed commit is `A`.

The depth gap is decomposed into powers of two.

Example:

```text
depth(B) - depth(A) = 13 = 8 + 4 + 1
```

Then jump:

```text
B --2^3--> X --2^2--> Y --2^0--> Z
```

Finally:

```text
Z == A  => A is an ancestor of B
Z != A  => A is not an ancestor of B
```

## Query Pseudocode

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

Where:

```text
ancestor(commit_id, exponent)
```

is a lookup in `ancestors`:

```sql
SELECT ancestor_id
  FROM ancestors
 WHERE commit_id = ?1
   AND exponent = ?2;
```

## Complexity

For each commit, the skip-list stores at most:

```text
floor(log2(first_parent_depth)) + 1
```

rows.

Ancestor checks use at most:

```text
O(log(depth(B) - depth(A)))
```

skip-list lookups.
