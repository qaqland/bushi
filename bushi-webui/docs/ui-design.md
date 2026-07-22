# UI Design

This is the authoritative UI specification for `bushi-webui`. Page sketches
are in [ux-prototype.md](ux-prototype.md); routes are in
[url-design.md](url-design.md).

## Principles

- All interface text is English. Repository content is displayed as authored.
- Read-only, server-rendered HTML with ordinary links; no JavaScript required.
- One centered column using Tailwind's `max-w-7xl` (80rem). No sidebar, nested
  cards, shadows, large rounded corners, or application-style menus.
- Repository navigation answers **where to go**; version and breadcrumb answer
  **where you are**; Content / History answers **how to view this path**.
- Keep directory listings cheap: no per-entry history or last-modified queries.
- Preserve browser back, refresh, deep links, keyboard access, and open-in-new-tab.
- Write the project name as `Bushi`. Lowercase `bushi` is only for identifiers
  (crate, binary, file names, URLs).

## Shared Navigation

The global bar contains `Bushi`, the repository name when known, and configured
external links. `Bushi` links to `/`; the repository name links to Overview.
External links have an external marker and use normal same-tab navigation.
Users can choose to open another tab using their browser.

Every known-repository HTML page has these four repository links:

```text
Overview    Files    Branches    Tags
```

There is **no repository-level History or Commits tab**.

- Overview always shows the default version.
- Files opens the root of the current version, or the default version if there
  is no version context.
- Branches and Tags are separate entries showing only their respective lists,
  using `/-/refs?kind=branches` and `/-/refs?kind=tags`. Their `rev` query retains
  the current version for the Files link; it does not imply switching mode.
- File, directory, and path-history pages mark Files as current.
- Commit details are standalone objects; no repository tab is marked current.
- Known-repository errors retain this navigation.

Path pages use three header rows: the global bar, repository navigation, then
one toolbar row holding the version control on the left, the breadcrumb, and
local view actions on the right:

```text
Bushi / bushi                                           GitHub ↗
Overview    Files    Branches    Tags
⎇ main ⇄   Root / src / main.rs               Content    History
```

The current version is a compact link with a thin border, small radius, and a
`⇄` marker, placed first in the toolbar row like a version selector. It opens
version selection, not a dropdown or an immediate version change. Its accessible
name and title describe switching and the current revision kind/name. Branches
use `⎇`; tags and commit snapshots retain visible `Tag` and `Commit` labels.
Names display Git slashes (`feature/login`), not URL colons (`feature:login`).

The snapshot hash links to commit details in secondary metadata below the page
content, not in the header. Content / History are boxed buttons matching the
version link; the current view is marked with the link color and weight instead
of another full-width tab bar. Long names and narrow screens wrap without
hiding navigation.

The current breadcrumb segment is plain text. Ancestors preserve the version
and current view: a history breadcrumb links to ancestor **history**, not to
ancestor content. Content / History preserves both version and path.

## Version Selection

The version link uses the existing `/-/refs` route with `rev`, `path`, and a
validated `view` parameter. There is no JavaScript dropdown or branch form.
Switching mode shows both branches and tags and marks neither repository list
entry as current.

Switching mode has the heading `Switch version`, an explanation of the path
and view being retained, a current-version label, and a Cancel link. Choosing
a row resolves the target revision and redirects to its canonical content or
history URL. Pagination cursors are not carried across versions.

- Preserve the path and content/history view.
- For content, use the actual target object kind to choose tree or blob.
- If content is absent or not a browsable file/directory, show a contextual
  error with the target root and `Return to original version` links.
- History may be available for deleted paths; do not reject it just because
  the target tree no longer contains the path.
- Normal Branches and Tags lists link names directly to version roots.
  The unfiltered `/-/refs` URL remains available for combined listings.
- Mark the default and current versions. No duplicate Tree or History actions
  appear alongside each name. Hashes still link to commit details.
- Empty branch/tag sections display `No branches.` / `No tags.` instead of
  empty table headers.

## Overview

Content order:

1. Repository name and meaningful Git description, when available.
2. A default-version link.
3. README or a concise empty state with Browse files.
4. Up to five recent commits, followed by View all commits.

Do not repeat the repository navigation as a Browse files / History / Refs
action bar. View all commits is a contextual content link to root history of
the default version, not another navigation tab.

README prose has a comfortable reading width. Markdown is sanitized before
insertion. Relative Markdown and HTML links resolve from the README's directory
at the displayed version; directory content links redirect to tree pages. Relative
images use the raw route. Local heading links have stable, deduplicated,
`readme-`-prefixed IDs. External URLs are not rewritten as repository paths.
Other files, including Markdown opened as a file, remain source views.

## Directory Content

Keep the exact column order and explicit Git markers:

```text
Mode      Name                         Size
040000    d ../                           --
040000    d page/                         --
100644    - main.rs                  3.1 KiB
```

**Mode stays visible. The `d` and `-` prefixes stay visible.** Markers are never
part of a URL. Directories sort first, have a trailing slash, and show `--` size.
Keep the parent-directory row as a convenient local return link.

Modes and sizes are visually secondary; names remain the main links. Show small
labels for executables, symbolic links, and submodules when applicable. Symlinks
show their stored target text without following filesystem paths. Submodules
are labeled, not misleading links to nonexistent local trees.

## File Content

Show filename, compact latest-change information, size, and text line count.
Both the latest-change subject and hash link to the commit.

- Text uses a line-numbered, unwrapped source table in a locally scrollable box.
- Each line has an `L<number>` ID. Its link uses the full resolved commit hash
  and `#L<number>`, so a moving branch cannot change a shared line reference.
- The target line is highlighted using CSS.
- Raw text and Download file are content actions, not history navigation.
- Empty text says `This file is empty.`
- Binary files show size and a download link, never a misleading zero line count.
- File titles and source code remain escaped, even for HTML or SVG files.

## History

The only path-view choices are Content and History. The heading explains scope:

- Root: `Commit history`
- Directory or file: `History of <path>`

Keep the exact table order:

```text
Commit      Subject                         Author       Time
4badb69a    Fix configuration loading        qaq          3d ago
```

**Hashes stay in the first column**, in smaller monospace type with stable
width. Subjects are normal-sized links; authors and times are secondary. Both
hash and subject link to the same commit.

History follows the index's first-parent chains and tracks paths by name; it
does not add rename-following or all-parent traversal. Current object kinds are
read from the selected Git tree, not inferred from globally shared path names.

Use cursor-based Older commits. Non-first pages also provide Latest commits,
which clears the cursor while retaining version and path. Empty results explain
that no matching commits were found at this version/path (and cursor, if used).

## Commit Details

- One primary subject heading, then author/email and an absolute authored date
  with timezone. Show the remaining message body without repeating the heading.
- Browse this version opens the commit snapshot root. Download patch downloads
  the complete patch. Do not expose an ambiguous History action here.
- Keep the full hash in secondary metadata. Every parent is a commit link.
- Initial commits say they have no parent. Merge commits identify the first
  parent as the comparison base; all parents are still visible.
- A changed-file summary precedes per-file diffs. Filenames jump to their own
  diff anchors; View file opens complete content at the relevant snapshot.
- Deleted files offer View before deletion at the comparison parent.
- Renames show old path → new path. Submodule changes are labeled, not blob links.
- Render at most 600 diff lines across the page. An explicit global notice and
  per-file placeholders identify truncation and link to the complete patch.
- Preserve literal `+` / `-` markers as well as color. Source and diff scroll
  horizontally inside their own regions, not across the whole page.

## Repository List and Errors

The root lists Name, Description, Updated. Only actual links are clickable,
not entire rows. Suppress Git's default `Unnamed repository` description. An
empty index has an explicit empty state.

Errors describe the requested path and version when available. Recovery links
lead to the version root, Branches & tags, or the original version after a
failed switch. Unknown repositories link back to the repository list. Never
expose stack traces, SQL errors, or internal filesystem paths.

## Visual and Accessibility Rules

- Use Tailwind CSS v4 with Preflight and the default palette and sizing scale.
  Light mode only; no dark theme or theme-switching controls.
- Body text uses `text-base` (1rem); tables, source and diff use `text-sm`
  (0.875rem). Hashes use `text-xs` (0.75rem) monospace. Navigation and prose use
  system fonts.
- Use default responsive breakpoints: horizontal padding changes from `px-4`
  to `md:px-8` at 48rem; navigation wraps naturally at any width.
- One restrained blue link color, readable muted metadata, thin section lines.
- No literal `[action]` brackets. Toolbar links use shared `.view-link` styling
  with a minimum height of `min-h-9`. Buttons and repository tabs never add text
  underlines on hover; buttons use background feedback for hover/press, while
  tabs use their existing bottom border. States must not shift layout.
- The current view retains its colored border, background and font weight during
  hover/press. Ordinary text links keep hover underlines and visited colors.
  Keyboard focus stays visible, including inset outlines on scrollable code links.
- Table headers and row borders are subtle; rows have usable vertical padding.
- README images fit the content width. Long tables and code scroll locally.
- At narrow widths, navigation and breadcrumbs wrap; the document must not
  require horizontal scrolling just to use navigation.
- Use labeled navigation, headings, table-header scopes, current-state semantics,
  a skip link, and visible keyboard focus. Never rely only on color or hover.

## Raw Content and Patch Safety

Raw and patch responses have no page frame. UTF-8 without NUL bytes is served
as `text/plain`, never executable HTML/SVG. Recognized PNG/JPEG/GIF/WebP bytes
use image content types so README images work. Other binaries download as
`application/octet-stream`. `?download=1` forces attachment disposition with an
encoded filename. Responses use `nosniff` and a restrictive sandbox CSP.
SVG execution, syntax highlighting, blame, search, clone controls, line-range
selection, and JavaScript navigation are not added by this redesign.

## Header Configuration

`bushi.toml` (or `BUSHI_CONFIG`) accepts ordered `header_links` with a nonempty
label and absolute HTTP(S) URL. The default is the Bushi GitHub project link;
setting the list replaces it, and `header_links = []` disables external links.
`BUSHI_DATABASE` and `BUSHI_BIND` remain scalar overrides. See the project README
and `bushi.toml.example` for runnable configuration.
