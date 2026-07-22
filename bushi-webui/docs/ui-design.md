# UI Design

This document describes the current UI direction for `bushi-webui`.

The page sketches live in `ux-prototype.md`. This document records the shared
rules, implementation choices, and minimum demo scope.

## Goals

- Serve useful repository pages with server-rendered HTML.
- Work without JavaScript.
- Keep routes predictable for browsers, curl, and AI agents.
- Use flat repository names, not `user/repo` namespaces.
- Prefer stable single-column pages over sidebars or split panes.
- Keep expensive Git history queries out of hot pages.
- Present a modern, business-like interface without rounded SaaS card styling.

## Visual Direction

- One main column.
- No side rail.
- No GitLab-style space-consuming sidebar.
- Rectangular sections with 1px borders.
- No shadows.
- No large rounded corners.
- High information density, but not dashboard-like.
- Monospace is used for paths, hashes, modes, code, and tabular Git data.
- Sans-serif can be used for prose, navigation, README content, and labels.

The intended feel is closer to a precise repository archive than a collaboration
platform.

## CSS Strategy

Use handwritten CSS.

Do not use Tailwind for the initial implementation.

Reasons:

- The UI is small and mostly static.
- Askama templates should remain readable.
- No Node, Tailwind build, purge step, or frontend toolchain is needed.
- A single stylesheet is enough for layout, tables, code, README, and navigation.
- Handwritten CSS gives tighter control over the rectangular visual language.

Suggested structure:

```text
static/
  bushi.css
```

Suggested CSS groups:

```text
variables
base elements
layout
header and navigation
sections
tables
code blocks
README rendering
footer
responsive rules
```

## Global Layout

Every HTML page should share the same basic frame:

```text
top header
horizontal rule
main content column
horizontal rule
footer
```

The main content column should stay stable across desktop and mobile. Responsive
behavior should mostly be natural vertical flow plus horizontal overflow for wide
tables or code blocks.

## Navigation

Repository pages use this top-level navigation, with `Summary` leftmost:

```text
Summary    Files    Refs
```

The entries are constant across all repository pages and never carry path
context. `About` no longer exists as a separate page; its role is folded into
Summary.

Header labels should stay short:

```text
bushi
bushi / @master
bushi / Refs
bushi / Commit / 4badb69a
```

Do not put long file paths in the header. Paths belong in the page body as a
breadcrumb section.

## Page Scope

The minimum demo consists of these HTML pages:

| Page | Route shape |
| --- | --- |
| Repository list | `/` |
| Summary | `/:repo` |
| Tree | `/:repo/-/tree/:rev/*path` |
| Blob | `/:repo/-/blob/:rev/*path` |
| Refs | `/:repo/-/refs` |
| History | `/:repo/-/history/:rev/*path` |
| Commit | `/:repo/-/commit/:hash` |
| Error | route-specific |

The minimum demo also exposes these non-HTML outputs:

| Output | Route shape |
| --- | --- |
| Raw blob | `/:repo/-/raw/:rev/*path` |
| Patch | `/:repo/-/commit/:hash.patch` |

Naming note: in the three-layer navigation, `Files` is the global-row entry
whose path-level view is `browse` (backed by tree/blob routes), and `history`
is the path-level view backed by history routes.

## Summary Page

Summary is the repository home page.

It should not duplicate tree, refs, or history content.

Behavior:

- Show a clone URL line above the README.
- Resolve README from the default revision.
- Render README server-side.
- Provide links to `blob`, `raw`, `history`, and `permalink` for the README.
- Show a short "recent commits" list below the README with a `-> more` link
  into the root history.
- If README is missing, show a small empty state with `tree` and `refs` links.
- Keep the rendered HTML allowlist strict and safe.

## Tree Page

Tree is a cheap directory listing.

The tree table should show:

- name
- mode
- size

The tree table must not show:

- last commit per entry
- last modified per entry
- author per entry

Reason: per-entry history data is noisy in the UI and expensive without caching.

Directories use size `--`. Directory names link to tree routes. File names link to
blob routes.

## Blob Page

Blob shows a single file at a selected revision.

Blob may show the latest change for that file because the query is scoped to one
path and is more reasonable than per-entry tree history.

Blob should include:

- path breadcrumb
- latest change row
- file metadata such as line count and size
- `raw`, `download`, `history`, and `permalink` links
- line-numbered code view

Code should use horizontal overflow instead of wrapping long lines.

Syntax highlighting is not required for the first version.

## Refs Page

Refs is the revision picker.

It lists branches and tags only.

It should not include direct commit lookup. Commit lookup can live elsewhere if
needed later.

Branch and tag names link to tree pages by default. Commit hashes in ref rows link

## History Page

There are two history views:

- repository history
- path-filtered history

Repository history shows commits reachable from the selected revision.

Path-filtered history shows commits that touched the selected file or directory.

The history table should not show changed files per commit.

Pagination uses cursor links:

```text
?after=<commit-hash>
```

Do not add page-number routes.

## Commit Page

Commit shows metadata, message, changed files, and diff.

It should include links to:

- `tree`
- `history`
- `patch`

The `tree` link opens the repository tree at that commit.

The `patch` link exposes a curl-friendly patch output.

The changed-file list appears before the diff so users can scan the commit before
reading the full patch.

Diff rendering can be omitted, limited, or paged later if backend cost becomes a
problem.

## Repository List Page

The repository list is the site root page.

It should show flat repository names only.

It should not imply user, organization, or namespace hierarchy.

Columns:

- name
- description (from the repository's `description` file)
- last updated (commit time of the default branch tip)

Optional data can be omitted if it becomes expensive.

## Error Page

Use a shared 404 layout for:

- repository not found
- revision not found
- path not found

If the repository is known, keep the repository navigation. If the repository is
unknown, show only the site name.

Do not expose stack traces, SQL errors, or internal paths.

## Hash Display

Commit hash examples and accepted hash route examples should use at least 8 hex
characters.

Shorter examples should not appear in UI documentation because the URL design
requires hash revisions to have length `>= 8`.

## No JavaScript Rule

All interactions in the initial UI should be normal links or server-rendered form
submissions.

Do not depend on JavaScript for:

- navigation
- revision switching
- pagination
- code viewing
- README rendering
- error recovery

## Text-Oriented Outputs

HTML pages should expose alternate text-oriented links where useful.

Examples:

```html
<link rel="alternate" type="text/markdown" href="...md">
<link rel="alternate" type="text/plain" href="...txt">
```

Raw and patch routes are not visual pages. They should return content directly
with suitable content types.
