# URL Design

Bushi uses flat repository names and a GitLab-style `/-/` operation boundary.
URLs are readable, shareable, server-rendered, and usable without JavaScript.
Navigation behavior is defined in [ui-design.md](ui-design.md).

## Repository and Operation Boundary

```text
/                         repository list
/:repo                    repository overview
/:repo/-/:operation/...   repository operation
```

Repository names do not have `user/repo` namespaces. Percent-encode repository
names and individual path segments, including spaces, Unicode, `#`, `?`, and
literal `%`. Preserve `/` as the path separator. HTML escaping is separate from
URL encoding. Application-generated URLs use the helpers in `src/url.rs`.

## Revision Syntax

```text
main                       branch
feature:login              branch feature/login
tag/v1.0                   tag v1.0
tag/release:2026-06         tag release/2026-06
abcdef12                   unambiguous commit hash prefix (8+ hex characters)
<full commit hash>         immutable commit identity
```

A branch or tag's name occupies one encoded path segment. The `tag/` discriminator
is a separate segment. The UI displays original Git names with slashes; colons
are an internal URL convention, not user-facing labels.

Parsing order remains tag discriminator, hexadecimal commit candidate, branch.
Hash-like branch names still collide with commit candidates; no new revision
syntax is introduced here. The SQLite adapter accepts both slash-containing
indexed names and older colon-form show names, exposing canonical URL names.

## HTML Routes

| Route | Purpose |
| --- | --- |
| `/` | Repository list |
| `/:repo` | Overview of the default version |
| `/:repo/-/refs?kind=branches` | Branches |
| `/:repo/-/refs?kind=tags` | Tags |
| `/:repo/-/refs` | Combined listing or context-preserving version selection |
| `/:repo/-/tree` | Temporary redirect to the default-version root |
| `/:repo/-/tree/:rev` | Version root |
| `/:repo/-/tree/:rev/*path` | Directory content |
| `/:repo/-/blob/:rev/*path` | File content |
| `/:repo/-/history/:rev` | Root commit history |
| `/:repo/-/history/:rev/*path` | Path history |
| `/:repo/-/commit/:hash` | Commit details and per-file diffs |

Blob content links that resolve to a directory redirect temporarily to the
corresponding tree URL. This also supports relative directory links in README
without guessing object kinds from filename extensions. `/blob/:rev` redirects
to the version root.

There is no separate diff route. Changed-file entries use `#diff-<n>` anchors
on the commit page. Text file lines have `#L<n>` anchors. Line-number links use
the full resolved commit hash so branch movement cannot change the reference.

## History Pagination

```text
/:repo/-/history/:rev/*path?after=<commit-hash>
```

The cursor is excluded from the next page. Older commits uses the next cursor;
Latest commits removes it without changing revision or path. There are no
page-number paths. Content/History switching and ancestor breadcrumbs do not
carry pagination cursors to a different scope.

## Branches & Tags Context

Separate listing entries:

```text
/bushi/-/refs?kind=branches&rev=feature%3Alogin
/bushi/-/refs?kind=tags&rev=feature%3Alogin
```

`kind` accepts only `branches` or `tags`; omitting it retains the combined
listing for existing links. `rev` retains the current version for the Files
navigation link and Current marker. Names still open version roots.

Explicit switching mode:

```text
/bushi/-/refs?rev=main&path=src%2Fmain.rs&view=blob
/bushi/-/refs?rev=main&path=src%2Fmain.rs&view=history
```

Parameters:

- `kind`: optional listing filter, `branches` or `tags`. Switching mode shows
  both kinds regardless of this filter.
- `rev`: source revision; defaults to the repository default when absent.
- `path`: source repository-relative path, or empty for root.
- `view`: exactly `tree`, `blob`, or `history`; its presence enables switching.
- `to`: target revision, added by a selection link.

Selection example:

```text
/bushi/-/refs?rev=main&path=src%2Fmain.rs&view=blob&to=feature%3Alogin
  -> /bushi/-/blob/feature:login/src/main.rs
```

The selection response validates the target and redirects to a normal path
URL, without switch parameters or pagination cursors. Content uses the target
object type. History does not require the path to exist in the target tree.
Missing content returns a contextual error with generated local recovery links;
it never silently redirects to the root.

Paths must be repository-relative, without empty, dot, parent, or NUL segments.
There is no arbitrary `return_to` URL or external redirect parameter. Query
values are encoded with the standard URL form serializer, independently of
route segments.

## Raw Content and Patch Downloads

```text
/:repo/-/raw/:rev/*path
/:repo/-/raw/:rev/*path?download=1
/:repo/-/commit/:hash.patch
```

- UTF-8 text without NUL bytes: `text/plain; charset=utf-8`.
- Recognized PNG, JPEG, GIF, WebP bytes: corresponding raster image type.
- Other binary content: `application/octet-stream`, attachment disposition.
- `download=1`: force attachment disposition, including for text and images.
- Patch: `text/x-patch; charset=utf-8`, attachment disposition, complete contents.

These responses have no HTML chrome. They use `X-Content-Type-Options: nosniff`
and a sandbox Content Security Policy. HTML/SVG source is not served as active
content. Filenames in attachment headers are UTF-8 percent-encoded.

## README Links

README links resolve relative to the README's directory at the displayed version.
They retain the default branch context rather than silently switching to a commit
snapshot; line-number permalinks are the explicit immutable navigation path.
Relative links use blob content URLs (directories redirect to tree); image
sources use raw URLs. Root-relative paths refer to the repository root, not to
the site root. External and protocol-relative URLs remain external. Local
heading fragments use prefixed, deduplicated IDs. Relative content-link query
strings are not forwarded as application control parameters.

## Debug Timing

Every response reports its server-side stage timings through the standard
`Server-Timing` response header, so no query parameter is involved and nothing is
added to the body. Raw content, patch downloads, and CSS carry the same header.
