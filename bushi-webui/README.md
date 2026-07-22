# bushi-webui

Web UI for browsing git repositories indexed by
[bushi-index](../bushi-index). Read-only: all commit/history data comes
from the SQLite database built by bushi-index, file contents are read
from the git repositories themselves.

## Prepare the database

bushi-webui never creates or syncs the database. Index a repository
first with bushi-index:

```sh
bushi-index -t test.db -a /path/to/repo
bushi-index -t test.db repo-name
```

The database file must exist before the server starts; a missing
database is a startup error.

## Run

```sh
cargo run
```

Environment variables:

| Variable         | Default            | Description            |
|------------------|--------------------|------------------------|
| `BUSHI_DATABASE` | `test.db`          | SQLite database path   |
| `BUSHI_BIND`     | `127.0.0.1:3000`   | listen address         |
| `RUST_LOG`       |                    | tracing filter         |

Then open http://127.0.0.1:3000/ for the repository list.

