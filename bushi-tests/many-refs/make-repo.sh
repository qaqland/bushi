#!/bin/sh
set -eu

cd "$(dirname "$0")"

total="${TOTAL:-100000}"
repo_dir="test-repo"

rm -rf "$repo_dir"
git init --quiet -b main "$repo_dir"

echo "make $repo_dir * $total refs"

python3 -c '
import sys

total = int(sys.argv[1])

assert 0 < total < 10000000

n_branch = total // 10
n_annotated = total * 4 // 10
n_lightweight = total - n_branch - n_annotated

print(f"""\
commit refs/heads/main
committer Test <test@qaq.land> 1700000000 +0000
data 7
initial
M 100644 inline my.txt
data 7
initial
""", end="")

for i in range(1, n_branch + 1):
    print(f"reset refs/heads/branch-{i:05d}")
    print("from refs/heads/main")

for i in range(1, n_lightweight + 1):
    print(f"reset refs/tags/tag-{i:05d}")
    print("from refs/heads/main")

for i in range(1, n_annotated + 1):
    print(f"""\
tag annotated-{i:05d}
from refs/heads/main
tagger Test <test@qaq.land> 1700000000 +0000
data 19
annotated tag {i:05d}
""", end="")
' "$total" | git -C "$repo_dir" fast-import --quiet
