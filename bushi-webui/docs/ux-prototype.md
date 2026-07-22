# UX Prototype

Sketches for the interface defined in [ui-design.md](ui-design.md).
All interface labels are English. `*` marks an active view; actions are ordinary
links. The interface works without JavaScript.

## Repository List — `/`

```text
Bushi                                                     GitHub ↗
------------------------------------------------------------------
Repositories

Name              Description                              Updated
bushi             Repository browsing                      3d ago
cgit              Hyperfast Git frontend                   2w ago
```

Names link to Overview. An empty index displays an explicit message rather
than an empty table.

## Overview — `/bushi`

```text
Bushi / bushi                                             GitHub ↗
------------------------------------------------------------------
Overview*    Files    Branches    Tags
------------------------------------------------------------------
bushi
Repository browsing
Default version: main

README
# bushi
A small, read-only Git browser.

## Usage
...

Recent commits
Commit      Subject                            Author      Time
4badb69a    Fix configuration loading           qaq         3d ago
3c2a1f0b    Extract page handlers               qaq         5d ago

View all commits
```

README comes before up to five recent commits. No duplicate action bar and no
repository-level History tab.

## Directory — `/bushi/-/tree/main/src`

```text
Bushi / bushi                                             GitHub ↗
------------------------------------------------------------------
Overview    Files*    Branches    Tags
------------------------------------------------------------------
⎇ main ⇄   Root / src                           Content*   History
------------------------------------------------------------------
Mode        Name                                             Size
040000      d ../                                              --
040000      d page/                                            --
100644      - main.rs                                     3.1 KiB
100755      - check.sh  Executable                        1.0 KiB
160000      - vendor  Submodule                               --
```

Mode and the `d` / `-` markers remain visible. A submodule is a labeled object,
not a broken local-directory link. The compact `⎇ main ⇄` link opens version
selection; it does not switch immediately. It sits first in the toolbar row,
left of the breadcrumb, like GitLab's branch selector. Path pages show a
secondary `Snapshot 4badb69a` commit link below the content, outside the header.

## File — `/bushi/-/blob/main/src/main.rs`

```text
Bushi / bushi                                             GitHub ↗
------------------------------------------------------------------
Overview    Files*    Branches    Tags
------------------------------------------------------------------
⎇ main ⇄   Root / src / main.rs                 Content*   History
------------------------------------------------------------------
main.rs                                  Raw text    Download file
Latest change: Fix configuration loading  4badb69a · qaq · 3d ago
172 lines · 5.8 KiB

  1  use axum::Router;
  2
  3  fn main() {
```

Line numbers link to immutable commit URLs with `#L<number>`. Binary files
show their size and download action, without a zero line count.

## History — `/bushi/-/history/main/src/main.rs`

```text
Bushi / bushi                                             GitHub ↗
------------------------------------------------------------------
Overview    Files*    Branches    Tags
------------------------------------------------------------------
⎇ main ⇄   Root / src / main.rs                 Content   History*
------------------------------------------------------------------
History of src/main.rs

Commit      Subject                            Author      Time
4badb69a    Fix configuration loading           qaq         3d ago
3c2a1f0b    Extract page handlers               qaq         5d ago

Older commits
```

The hash stays first, in smaller monospace text. Clicking `src` shows the
history of `src`, not its content. On later pages, Latest commits appears next
to Older commits. Root history uses the heading Commit history.

## Branches — `/bushi/-/refs?kind=branches&rev=main`

```text
Bushi / bushi                                             GitHub ↗
------------------------------------------------------------------
Overview    Files    Branches*    Tags
------------------------------------------------------------------
Branches
Name                                                    Commit
main  Default  Current                                  4badb69a
feature/login                                           3c2a1f0b
```

## Tags — `/bushi/-/refs?kind=tags&rev=main`

```text
Bushi / bushi                                             GitHub ↗
------------------------------------------------------------------
Overview    Files    Branches    Tags*
------------------------------------------------------------------
Tags
No tags.
```

Each entry shows only its own list. Names open version roots. Hashes open
commits. There are no redundant Tree or History links per row.

## Switch Version — `/bushi/-/refs?rev=main&path=src/main.rs&view=blob`

```text
Bushi / bushi                                             GitHub ↗
------------------------------------------------------------------
Overview    Files    Branches    Tags
------------------------------------------------------------------
Switch version
Continue viewing content of src/main.rs. Current version: main.
Cancel

Branches
Name                                                    Commit
main  Default  Current                                  4badb69a
feature/login                                           3c2a1f0b

Tags
Name                                                    Commit
v1.0                                                    5f6e7d8f
```

Selection preserves path and view, then redirects to a normal tree/blob/history
URL. Cancel returns to the original context. Switching from History retains
History, including for deleted paths.

## Failed Switch

```text
Bushi / bushi                                             GitHub ↗
------------------------------------------------------------------
Overview    Files    Branches    Tags
------------------------------------------------------------------
404 Not Found
src/main.rs at release
path not found

Browse version root    Branches & tags    Return to original version
```

No silent redirect to the root when the requested content is missing.

## Commit — `/bushi/-/commit/9e8d7c6d`

```text
Bushi / bushi                                             GitHub ↗
------------------------------------------------------------------
Overview    Files    Branches    Tags
------------------------------------------------------------------
Fix configuration loading
9e8d7c6d · qaq <qaq@example.com> · 2026-07-14 09:32:11 +08:00
Browse this version    Download patch

The loader now reports invalid configuration before startup.

Commit     9e8d7c6d1a2b3c4e5f60718293a4b5c6d7e8f901
Parents    3c2a1f0b

Changes
2 files changed · +13 −3

File                 Status       Add    Del    Content
src/config.rs        modified     +13    −2     View file
old.conf             deleted       +0    −1     View before deletion

src/config.rs                                          View file
@@ -41,6 +41,7 @@
...
```

Filenames in Changes jump to the corresponding diff. Every parent is linked;
merge pages name the first parent as the comparison base. A 600-line display
limit is explicitly announced, with a complete patch download.

## Narrow Screens and Keyboard Use

Keep the same information hierarchy. Navigation and breadcrumbs wrap, while
source and tables scroll inside their own regions. A skip link and visible
focus support keyboard navigation; active views and target lines have explicit
semantics and styling. No separate mobile menu is needed.
