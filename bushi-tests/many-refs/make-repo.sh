#!/bin/sh
set -eu

cd "$(dirname "$0")"

total="${TOTAL:-100000}"
repo_dir="test-repo"

rm -rf "$repo_dir"
git init --quiet -b main "$repo_dir"

echo "make $repo_dir * $total refs"

if [ "$total" -le 0 ] || [ "$total" -ge 10000000 ]; then
    echo "TOTAL out of range" >&2
    exit 1
fi

awk -v total="$total" 'BEGIN {
    n_branch = int(total / 10)
    n_annotated = int(total * 4 / 10)
    n_lightweight = total - n_branch - n_annotated

    printf "commit refs/heads/main\n"
    printf "committer Test <test@qaq.land> 1700000000 +0000\n"
    printf "data 7\n"
    printf "initial\n"
    printf "M 100644 inline my.txt\n"
    printf "data 7\n"
    printf "initial\n"

    for (i = 1; i <= n_branch; i++) {
        printf "reset refs/heads/branch-%05d\n", i
        printf "from refs/heads/main\n"
    }
    for (i = 1; i <= n_lightweight; i++) {
        printf "reset refs/tags/tag-%05d\n", i
        printf "from refs/heads/main\n"
    }
    for (i = 1; i <= n_annotated; i++) {
        printf "tag annotated-%05d\n", i
        printf "from refs/heads/main\n"
        printf "tagger Test <test@qaq.land> 1700000000 +0000\n"
        printf "data 19\n"
        printf "annotated tag %05d\n", i
    }
}' | git -C "$repo_dir" fast-import --quiet
