# Path History Query Algorithm

## Overview

bushi-index precomputes a `last_commit_id` chain for every path (both files
and directories), recording which commit previously touched that path.
Queries follow these precomputed links directly from one relevant commit to
the next, avoiding a linear scan of the entire first-parent chain.

## Unified Path Model

Files and directories share the same `paths` table, distinguished only by a
trailing slash: `src/main.c` is a file, `src/` is a directory.
Because both kinds of records carry a `last_commit_id` chain, a single query
algorithm handles files and directories identically — no special-case logic
is needed for directory traversal.

## Binary-Lifted Ancestor Lookup

Before following the `last_commit_id` chain, the query must locate the
nearest commit on the first-parent chain that actually modified the
requested path.

The index stores a precomputed 2^n ancestor table (similar to the binary
lifting technique in competitive programming).  Candidates are scanned in
descending depth order from the `changes` table, and each candidate is
verified in O(log n) using the 2^n skip list — walk the ancestor table by
decreasing exponents, comparing depths, to check whether the candidate lies
on the first-parent chain of the starting commit.

This avoids walking the chain commit-by-commit and makes the start-point
search proportional to the number of candidates checked, not the total
chain length.

## Chain-Based History Traversal

Once the starting commit is found, the query follows the `last_commit_id`
links: each row points directly to the previous commit that touched the same
path.  The traversal visits only commits relevant to the path, so its cost
scales with the result count rather than the repository size.

## Self-Referencing Sentinel

When a path appears for the first time, its `last_commit_id` equals its own
`commit_id`.  The recursive traversal treats this as a terminal condition
and stops automatically — no separate boundary check or sentinel value is
needed.

## Early Termination via In-CTE LIMIT

The `LIMIT` clause is placed inside the recursive CTE rather than outside.
This lets SQLite halt recursion as soon as enough rows are produced, while
still guaranteeing correct ordering.  The effect is that a bounded query
(e.g. `-n 20`) only traverses as many links as the requested count.

## Caveat

For paths modified extremely frequently (e.g. top-level directories), the
start-point search must scan many candidate commits before finding one on
the first-parent chain, which can make the lookup slower.

