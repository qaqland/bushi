# Backfill First-Parent Depth

## Goal

During backfill, compute and persist each commit's depth along the first-parent
chain to the root commit.

Depth definition:

```text
depth(root) = 0
depth(commit) = depth(first_parent(commit)) + 1
```

For example:

```text
A(root) <- B <- C <- D

depth(A) = 0
depth(B) = 1
depth(C) = 2
depth(D) = 3
```

## Schema

Add a depth column to `commits`:

```sql
ALTER TABLE commits ADD COLUMN first_parent_depth INTEGER;
```

For a fresh database, the table definition should include:

```sql
CREATE TABLE IF NOT EXISTS commits
(      commit_id           INTEGER PRIMARY KEY AUTOINCREMENT
     , commit_hash         TEXT    NOT NULL
     , parent_hash         TEXT                    -- only first parent
     , first_parent_depth  INTEGER
     , repository_id       INTEGER NOT NULL
) STRICT;
```

`first_parent_depth` is nullable so newly inserted commits can be written first
and filled during the later backfill phase.

## Backfill Data Structures

Extend `struct backfill_index` with depth storage:

```c
struct backfill_index {
	uint32_t num_commits;

	struct idmap idmap; // global commit_id -> commit local_idx

	// input commit local_idx
	int64_t *commit_ids;     // -> global commit_id
	uint32_t *parent_local;  // -> parent local_idx (UINT32_MAX = none)
	uint32_t *fp_depth;      // -> first-parent depth (UINT32_MAX = unknown)
};
```

Use `UINT32_MAX` as the in-memory sentinel for an unknown depth.

Use a reusable heap-backed stack for the temporary path:

```c
struct uint32_stack {
	uint32_t *items;
	size_t nr;
	size_t alloc;
};
```

Push with Git's dynamic array macro:

```c
ALLOC_GROW(stack->items, stack->nr + 1, stack->alloc);
stack->items[stack->nr++] = value;
```

Pop with:

```c
value = stack->items[--stack->nr];
```

Before computing a new commit, reset only the length:

```c
stack->nr = 0;
```

Free once after all depths are computed:

```c
free(stack->items);
```

## Algorithm

After `build_backfill_index()` finishes building `parent_local[]`, compute all
depths from that array.

For each local commit index `i`:

1. If `fp_depth[i]` is already known, skip it.
2. Walk from `i` along `parent_local[]` while the current commit has unknown
   depth.
3. Push each unknown commit into the temporary path stack.
4. Stop when reaching either `UINT32_MAX` root or a commit with known depth.
5. Walk the path stack backwards and assign depths.

Pseudocode:

```c
for (uint32_t i = 0; i < idx->num_commits; i++) {
	uint32_t curr = i;
	uint32_t depth;

	if (idx->fp_depth[i] != UINT32_MAX)
		continue;

	path.nr = 0;

	while (curr != UINT32_MAX && idx->fp_depth[curr] == UINT32_MAX) {
		ALLOC_GROW(path.items, path.nr + 1, path.alloc);
		path.items[path.nr++] = curr;
		curr = idx->parent_local[curr];
	}

	if (curr == UINT32_MAX)
		depth = 0;
	else
		depth = idx->fp_depth[curr] + 1;

	while (path.nr) {
		uint32_t v = path.items[--path.nr];
		idx->fp_depth[v] = depth++;
	}
}
```

This is iterative and does not use recursion, so it can handle first-parent
chains with depths in the tens of millions.

## Persisting Depths

Add a prepared statement for writing the computed depth:

```sql
UPDATE commits
   SET first_parent_depth = ?1
 WHERE commit_id = ?2;
```

After computing `fp_depth[]`, write each local commit's depth back using
`commit_ids[i]`:

```c
for (uint32_t i = 0; i < idx->num_commits; i++)
	update_first_parent_depth(idx->commit_ids[i], idx->fp_depth[i]);
```

This should run in the existing backfill transaction.

## Memory Characteristics

For `N` commits:

```text
parent_local: N * sizeof(uint32_t)
fp_depth:     N * sizeof(uint32_t)
path stack:   up to max_depth * sizeof(uint32_t)
```

For a first-parent chain of 10,000,000 commits, the path stack peaks at about
40 MB.

## Runtime Characteristics

Each commit receives a depth once. Once a depth is assigned, later walks stop at
that commit instead of continuing to the root.

The intended runtime is linear in the number of commits plus the number of
first-parent links traversed before caching takes effect.
