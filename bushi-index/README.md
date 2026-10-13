# bushi-index

Index local Git repository metadata into an SQLite database.

## Build

Requires a C23 compiler with `#embed` support (e.g. GCC 15+), Make, Meson 1.3+,
Ninja, and development packages for SQLite 3.37+ and zlib.

```sh
make libgit   # download Git source and compile libgit.a
meson setup build
meson compile -C build
```

## Usage

```sh
export BUSHI_DATABASE="$PWD/index.db"    # created automatically; use -t to override

./build/bushi-index -a /path/to/my-repo  # register a repository
./build/bushi-index my-repo              # initial or incremental sync
./build/bushi-index -l                   # list registered repositories
./build/bushi-index -s my-repo           # show statistics
```
