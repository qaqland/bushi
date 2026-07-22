# UX Prototype

ASCII prototypes for the three-layer navigation, filled with dummy data.

Repo: `bushi`   Branch: `main`   Head: `4badb69a`   Author: `qaq`

## Layer Legend

```text
Global row  - summary | files | refs      right: current ref/hash
Object row  - shown only when viewing a concrete commit
Path row    - breadcrumb + view switcher, history always rightmost

~ / ◇ / ●   branch / tag / commit symbols (use text-safe: ~ = branch)
```

## View 1: Index (repository list) — `GET /`

```text
+------------------------------------------------------------------------------+
|  bushi : index                                                               |
|      AI code creation and repository hosting                                 |
+------------------------------------------------------------------------------+
|  Repositories                                                                |
|  +--------------------------------------------------------------------------+|
|  | Name              Head        Commit    Description                      ||
|  +--------------------------------------------------------------------------+|
|  | bushi             main        4badb69a  AI code creation and browsing    ||
|  | cgit              master      8f3e2d1c  hyperfast web frontend for git   ||
|  | wlroots           master      a1b2c3d4  modular wayland compositor lib   ||
|  +--------------------------------------------------------------------------+|
+------------------------------------------------------------------------------+
|                          bushi 0.0.1   render: 2ms                           |
+------------------------------------------------------------------------------+
```

## View 2: Summary (default landing, has README) — `GET /bushi`

```text
+------------------------------------------------------------------------------+
|  bushi : bushi                                                               |
|      AI code creation and repository browsing                                |
+------------------------------------------------------------------------------+
|  summary*  files   refs                                  ~ main ● 4badb69a  |
+------------------------------------------------------------------------------+
|  root                [ browse ]                                    [history] |
+------------------------------------------------------------------------------+
|  clone: https://git.example.com/bushi.git                                    |
|                                                                              |
|  # bushi                                                                     |
|                                                                              |
|  AI code creation and repository browsing.                                   |
|                                                                              |
|  ## Usage                                                                    |
|                                                                              |
|      bushi-webui --database bushi.db                                        |
|                                                                              |
|  recent commits                                                              |
|  +--------------------------------------------------------------------------+|
|  | ● 4badb69a  tests: update git-log.sql                 qaq    5 days ago ||
|  | ● 3c2a1f0b  feat: add channel module                  qaq     1 week ago ||
|  | ● 9e8d7c6d  fix: memory leak in parser               qaq    2 weeks ago ||
|  +--------------------------------------------------------------------------+|
|  -> more                                                                     |
+------------------------------------------------------------------------------+
|                          bushi 0.0.1   render: 5ms                           |
+------------------------------------------------------------------------------+
```

## View 3: Files, directory browse — `GET /bushi/-/tree/main/src`

```text
+------------------------------------------------------------------------------+
|  bushi : bushi                                                               |
|      AI code creation and repository browsing                                |
+------------------------------------------------------------------------------+
|  summary   files*  refs                                  ~ main ● 4badb69a  |
+------------------------------------------------------------------------------+
|  root / src          [ browse*]                                    [history] |
+------------------------------------------------------------------------------+
|  +--------------------------------------------------------------------------+|
|  | Name                                   Mode       Size                   ||
|  +--------------------------------------------------------------------------+|
|  | ../                                    040000     --                     ||
|  | database/                              040000     --                     ||
|  | server/                                040000     --                     ||
|  | templates/                             040000     --                     ||
|  | main.rs                                100644     3.1 KiB                ||
|  | routes.rs                              100644     7.8 KiB                ||
|  | state.rs                               100644     1.4 KiB                ||
|  +--------------------------------------------------------------------------+|
+------------------------------------------------------------------------------+
|                          bushi 0.0.1   render: 4ms                           |
+------------------------------------------------------------------------------+
```

## View 4: Files, file browse — `GET /bushi/-/blob/main/src/ui-shared.c`

```text
+------------------------------------------------------------------------------+
|  bushi : bushi                                                               |
|      AI code creation and repository browsing                                |
+------------------------------------------------------------------------------+
|  summary   files*  refs                                  ~ main ● 4badb69a  |
+------------------------------------------------------------------------------+
|  root / src / ui-shared.c    [ browse*] [ raw ]                    [history] |
+------------------------------------------------------------------------------+
|  latest: ● 3c2a1f0b  feat: add channel module           qaq    1 week ago   |
|  88 lines   2.4 KiB                              [permalink]                 |
|  +--------------------------------------------------------------------------+|
|  |   1  #include "ui-shared.h"                                              ||
|  |   2  #include <stdlib.h>                                                 ||
|  |   3                                                                     ||
|  |   4  void ui_render_header(struct ctx *c)                               ||
|  |   5  {                                                                  ||
|  |   6      html(c, "<header>");                                           ||
|  |   7      /* ... */                                                       ||
|  |   8  }                                                                  ||
|  +--------------------------------------------------------------------------+|
+------------------------------------------------------------------------------+
|                          bushi 0.0.1   render: 6ms                           |
+------------------------------------------------------------------------------+
```

## View 5: History, path filtered — `GET /bushi/-/history/main/src`

```text
+------------------------------------------------------------------------------+
|  bushi : bushi                                                               |
|      AI code creation and repository browsing                                |
+------------------------------------------------------------------------------+
|  summary   files*  refs                                  ~ main ● 4badb69a  |
+------------------------------------------------------------------------------+
|  root / src          [ browse ]                                   [history*] |
+------------------------------------------------------------------------------+
|  +--------------------------------------------------------------------------+|
|  | Commit    Subject                                   Author     Time      ||
|  +--------------------------------------------------------------------------+|
|  | ● 4badb69a  tests: update git-log.sql               qaq     5 days ago  ||
|  | ● 3c2a1f0b  feat: add channel module                qaq      1 week ago ||
|  | ● 9e8d7c6d  fix: memory leak in parser             qaq     2 weeks ago ||
|  | ● 1a2b3c4e  docs: update README                    qaq     3 weeks ago ||
|  +--------------------------------------------------------------------------+|
|  older                                                                       |
+------------------------------------------------------------------------------+
|                          bushi 0.0.1   render: 5ms                           |
+------------------------------------------------------------------------------+
```

## View 6: Commit page — `GET /bushi/-/commit/9e8d7c6d`

```text
+------------------------------------------------------------------------------+
|  bushi : bushi                                                               |
|      AI code creation and repository browsing                                |
+------------------------------------------------------------------------------+
|  summary   files*  refs                                  ~ main ● 4badb69a  |
+------------------------------------------------------------------------------+
|  ● 9e8d7c6d  fix: memory leak in parser              [ commit*] [ patch ]     |
+------------------------------------------------------------------------------+
|  root / src / parser.c     [ browse ] [ raw ]                      [history] |
+------------------------------------------------------------------------------+
|  author   qaq <qaq@example.com>                                              |
|  date     2026-07-14 09:32:11 +0800                                          |
|  parent   ● 3c2a1f0b                                                         |
|                                                                              |
|  fix: memory leak in parser                                                  |
|                                                                              |
|  The parse context was never freed on the error path.                        |
|                                                                              |
|  changed files                                                               |
|  +--------------------------------------------------------------------------+|
|  | src/parser.c            +12   -3                                         ||
|  | src/parser.h            + 1   -0                                         ||
|  +--------------------------------------------------------------------------+|
|  diff                                                                        |
|  +--------------------------------------------------------------------------+|
|  | @@ -41,6 +41,7 @@ int parse_stream(struct parse_ctx *ctx)               ||
|  |       if (err) {                                                         ||
|  | -        return -1;                                                      ||
|  | +        parse_ctx_free(ctx);                                            ||
|  | +        return -1;                                                      ||
|  |       }                                                                  ||
|  +--------------------------------------------------------------------------+|
+------------------------------------------------------------------------------+
|                          bushi 0.0.1   render: 7ms                           |
+------------------------------------------------------------------------------+
```

## View 7: Patch (plain text output) — `GET /bushi/-/commit/9e8d7c6d.patch`

```text
From 9e8d7c6d1a2b3c4e5f60718293a4b5c6d7e8f901 Mon Sep 17 00:00:00 2001
From: qaq <qaq@example.com>
Date: Tue, 14 Jul 2026 09:32:11 +0800
Subject: [PATCH] fix: memory leak in parser

The parse context was never freed on the error path.
---
 src/parser.c | 15 ++++++++++++---
 src/parser.h |  1 +
 2 files changed, 13 insertions(+), 3 deletions(-)
...

Content-Type: text/plain; charset=utf-8   (no page chrome, curl-friendly)
```

## View 8: Raw blob (plain text output) — `GET /bushi/-/raw/main/src/ui-shared.c`

```text
#include "ui-shared.h"
#include <stdlib.h>
...

Content-Type: text/plain; charset=utf-8   (no page chrome, curl-friendly)
```

## View 9: Refs page — `GET /bushi/-/refs`

```text
+------------------------------------------------------------------------------+
|  bushi : bushi                                                               |
|      AI code creation and repository browsing                                |
+------------------------------------------------------------------------------+
|  summary   files   refs*                                 ~ main ● 4badb69a  |
+------------------------------------------------------------------------------+
|  root                [ browse ]                                    [history] |
+------------------------------------------------------------------------------+
|  branches                                                                    |
|  +--------------------------------------------------------------------------+|
|  | ~ main                              ● 4badb69a   tests: update git-log  ||
|  | ~ feature:login                     ● 1a2b3c4e   docs: update README    ||
|  +--------------------------------------------------------------------------+|
|  tags                                                                        |
|  +--------------------------------------------------------------------------+|
|  | ◇ v0.1.0                            ● 5f6e7d8f   init: project setup   ||
|  +--------------------------------------------------------------------------+|
+------------------------------------------------------------------------------+
|                          bushi 0.0.1   render: 3ms                           |
+------------------------------------------------------------------------------+
```

## View 10: 404 (known repository) — `GET /bushi/-/tree/main/nope`

```text
+------------------------------------------------------------------------------+
|  bushi : bushi                                                               |
|      AI code creation and repository browsing                                |
+------------------------------------------------------------------------------+
|  summary   files*  refs                                  ~ main ● 4badb69a  |
+------------------------------------------------------------------------------+
|  root / src / nope     [ browse ] [ raw ]                          [history] |
+------------------------------------------------------------------------------+
|  404  not found                                                              |
|                                                                              |
|  The path "src/nope" does not exist at revision "main".                      |
|                                                                              |
|  Links:  summary   files   refs                                              |
+------------------------------------------------------------------------------+
|                          bushi 0.0.1   render: 1ms                           |
+------------------------------------------------------------------------------+
```

## Link Map

```text
summary          /bushi
files            /bushi/-/tree/main
refs             /bushi/-/refs
ref name (main)  /bushi/-/tree/main
head hash        /bushi/-/commit/4badb69a
copy             full hash to clipboard (browser only; plain link in w3m)
breadcrumb seg   /bushi/-/tree/main/<seg>
browse           /bushi/-/tree/main/<path>   or  /bushi/-/blob/main/<path>
raw              /bushi/-/raw/main/<path>
history          /bushi/-/history/main/<path>
older            /bushi/-/history/main/<path>?after=<hash>
commit           /bushi/-/commit/<hash>
patch            /bushi/-/commit/<hash>.patch
permalink        same route with branch resolved to full hash
```

## Notes

- Global row order is fixed: summary | files | refs. It never carries path
  context; clicking any of them leaves the current path.
- Default landing: README present -> summary; no README -> files.
- Summary is the repository home: clone URL line, rendered README, recent
  commits, and a "-> more" link into the root history.
- Object row appears only on commit and patch pages; it always shows the
  opened object, while the global row right side always shows the ref head.
- The commit page includes the full diff below the changed-file list; there is
  no separate diff view.
- Breadcrumb segments link to browse at that depth.
- history stays rightmost on the path row regardless of path depth.
- blame and the header branch-switch form are removed.
- All views render legibly in w3m/links2/elinks: no JS, no CSS dependence,
  tables degrade to linear text.
- raw and patch outputs have no page chrome; they are plain text for curl.
