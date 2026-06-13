#!/bin/sh
set -eu

cd "$(dirname "$0")"

total="${TOTAL:-100000}"
repo_dir="test-repo"

rm -rf "$repo_dir"
git init --quiet -b main "$repo_dir"

echo "make $repo_dir * $total"

python3 -c '
import os
import sys

total = int(sys.argv[1])
interval = 10

assert 0 < total < 10000000

for i in range(1, total + 1):
    path = "my.txt" if i % interval == 0 else f"{i % 100}.txt"
    message = f"m-{i:07d}"
    content = f"c-{i:07d}"
    timestamp = 1700000000 + i

    print(f"""\
commit refs/heads/main
committer Test <test@qaq.land> {timestamp} +0000
data 9
{message}
M 100644 inline {path}
data 9
{content}

""", end="")
' "$total" | git -C "$repo_dir" fast-import --quiet
