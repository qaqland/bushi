# URL Design

`bushi-webui` uses flat repository names and GitLab-style `/-/` operation
separation. URLs should be easy to read in browsers, stable enough for curl, and
unambiguous for AI agents.

## Goals

```text
flat repository names only
clear operation boundary with /-/
revision names that fit in one path segment
no JavaScript requirement
curl-friendly links
AI-friendly, predictable route shapes
```

## Repository Boundary

Only flat repository names are supported:

```text
/:repo
```

All server operations live after `/-/`:

```text
/:repo/-/:operation/...
```

Everything before `/-/` identifies the repository. Everything after `/-/` is
owned by `bushi-webui`.

Examples:

```text
/cgit
/cgit/-/refs
/cgit/-/history/master
/cgit/-/blob/master/README.md
```

## Revision Syntax

Routes that need a revision use one path segment named `:rev`.

Supported revision forms:

```text
tag/<tag-show-name>  tag
<hash>               commit hash, only if length >= 8 and hex-only
<branch-show-name>   branch
```

Branch and tag show names replace `/` in the Git ref name with `:` so the whole
revision remains one URL path segment.

Examples:

```text
refs/heads/master              -> master
refs/heads/feature/login       -> feature:login
refs/tags/v1.0                 -> tag/v1.0
refs/tags/release/2026-06      -> tag/release:2026-06
abcdef12                       -> commit hash
```

Parsing order:

```text
1. if rev starts with tag/, parse as tag
2. else if len(rev) >= 8 and rev is hex-only, parse as commit hash
3. else parse as branch
```

There is no explicit branch fallback. If a branch show name conflicts with a
commit hash candidate, the commit hash wins.

## Core Routes

Repository summary:

```text
/:repo
```

References:

```text
/:repo/-/refs
```

Commit detail:

```text
/:repo/-/commit/:hash
```

Tree browser:

```text
/:repo/-/tree/:rev
/:repo/-/tree/:rev/*path
```

Rendered blob page:

```text
/:repo/-/blob/:rev/*path
```

Raw blob content:

```text
/:repo/-/raw/:rev/*path
```

Commit history:

```text
/:repo/-/history/:rev
/:repo/-/history/:rev/*path
```

## Query Parameters

The initial public query parameter is only:

```text
?after=<commit-hash>
```

`after` is used for cursor-based log pagination. It means the next page should
continue after the given commit and should not include that commit again.

Examples:

```text
/cgit/-/history/master?after=abcdef1234567890
/cgit/-/history/master/README.md?after=abcdef1234567890
/cgit/-/history/tag/v1.0?after=abcdef1234567890
```

Do not add page-number path segments such as:

```text
/cgit/-/history/master/p2
/cgit/-/history/master/page/2
```

## Examples

Branch tree:

```text
/cgit/-/tree/master
/cgit/-/tree/feature:login/src
```

Tag tree:

```text
/cgit/-/tree/tag/v1.0
/cgit/-/tree/tag/release:2026-06/src
```

Commit tree:

```text
/cgit/-/tree/abcdef12
```

File pages:

```text
/cgit/-/blob/master/README.md
/cgit/-/raw/master/README.md
/cgit/-/history/master/README.md
```

Paginated history:

```text
/cgit/-/history/master?after=abcdef1234567890
```
