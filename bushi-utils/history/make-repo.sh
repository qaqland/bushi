#!/bin/sh
set -eu

cd "$(dirname "$0")"

total="${TOTAL:-100000}"
repo_dir="test-repo"

rm -rf "$repo_dir"
git init --quiet -b main "$repo_dir"

echo "make $repo_dir * $total"

if [ "$total" -le 0 ] || [ "$total" -ge 10000000 ]; then
    echo "TOTAL out of range" >&2
    exit 1
fi

awk -v total="$total" 'BEGIN {
    interval = 10
    for (i = 1; i <= total; i++) {
        path = (i % interval == 0) ? "my.txt" : sprintf("%d.txt", i % 100)
        message = sprintf("m-%07d", i)
        content = sprintf("c-%07d", i)
        timestamp = 1700000000 + i

        printf "commit refs/heads/main\n"
        printf "committer Test <test@qaq.land> %d +0000\n", timestamp
        printf "data 9\n"
        printf "%s\n", message
        printf "M 100644 inline %s\n", path
        printf "data 9\n"
        printf "%s\n", content
        printf "\n"
    }
}' | git -C "$repo_dir" fast-import --quiet
