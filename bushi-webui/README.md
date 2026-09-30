# bushi-webui

A read-only web interface for browsing Git repositories indexed by
[bushi-index](../bushi-index).

## Build

```sh
cargo build --release
```

## Run

The server needs a SQLite database created by `bushi-index`. Copy
`bushi.toml.example` to `bushi.toml` and point `database` at it:

```toml
database = "test.db"
bind = "127.0.0.1:3000"
```

Then start the server:

```sh
cargo run --release
```

Open http://127.0.0.1:3000 in a browser.

## Styles

`static/bushi.css` is generated from `styles/bushi.css` and compiled into the
binary. To change styles, edit the templates or `styles/bushi.css`, then run:

```sh
npm ci
npm run css:build
cargo build
```

The generated file is committed; do not edit it by hand.
