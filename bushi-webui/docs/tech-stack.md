# Tech Stack

`bushi-webui` is the web hosting service for Bushi. It serves repository
content from the SQLite index produced by `bushi-index`, with browser-friendly
HTML and curl/AI-friendly textual representations.

## Runtime

Use `tokio` as the async runtime.

Responsibilities:

```text
network IO
request concurrency
graceful shutdown
```

## HTTP Server

Use `axum` for HTTP routing and request handling.

Responsibilities:

```text
route definitions
path and query extraction
shared application state
error mapping to HTTP responses
content negotiation or explicit format suffix handling
```

URL structure is documented in `docs/url-design.md`.

## HTML Rendering

Use `askama` for server-side HTML templates.

Responsibilities:

```text
compile-time checked templates
layout and partial reuse
HTML escaping
no-JS browser pages
```

HTML pages should expose alternate text-oriented links where useful:

```html
<link rel="alternate" type="text/markdown" href="...md">
<link rel="alternate" type="text/plain" href="...txt">
```

## Styles

Use Tailwind CSS v4 CLI to generate `static/bushi.css` from `styles/bushi.css`
and the Askama templates. Use the default palette, spacing, typography and
responsive breakpoints; support light mode only for now.

Use utilities in templates for layout and small adjustments. Keep shared table,
Markdown, source-code, diff and navigation-state rules in the CSS input. Preflight
is enabled; Markdown headings, lists and other prose defaults are restored there.
No frontend framework, bundler, browser-side Tailwind runtime or CDN is needed.

Node.js is a development/build dependency only. Commit the generated CSS so a
plain Cargo build remains sufficient for consumers. Release builds regenerate
CSS before compiling Rust: Axum embeds the stylesheet with `include_str!`, so
CSS watch mode alone cannot update an already-running server. Commands and
maintenance conventions are documented in `README.md`.

## SQLite Access

Use `deadpool` for SQLite connection pooling.

Responsibilities:

```text
bounded database concurrency
read-only pooled connections
busy timeout handling
keeping blocking SQLite work off async request flow where appropriate
```

The server should treat the database as read-only:

```sql
PRAGMA query_only = ON;
PRAGMA foreign_keys = ON;
PRAGMA busy_timeout = 5000;
```

`bushi-webui` does not trigger, schedule, or orchestrate `bushi-index`. Indexing
is handled by other services outside this server.

## Cache

Use `moka` for in-process caching.

Responsibilities:

```text
hot repository metadata
ref resolution
small query results
render-independent data objects
```

Initial cache candidates:

```text
repository lookup by name
ref lookup by repository and show name
commit lookup by hash prefix or full hash
file path to file_id lookup
small recent log pages
```

Cache entries should be safe to drop at any time. SQLite remains the source of
truth.

## Boundaries

`bushi-webui` only serves web pages and related textual representations.

```text
bushi-index   -> sync Git repository metadata into SQLite
other service -> invoke or schedule bushi-index
bushi-webui   -> read SQLite and host web pages
```

It should not perform indexing work, expose indexing controls, or own background
sync jobs.

Request handlers should avoid direct SQL. Prefer this shape:

```text
handler -> service -> db query functions -> SQLite
```

Templates should receive already-shaped view models, not raw database rows.
