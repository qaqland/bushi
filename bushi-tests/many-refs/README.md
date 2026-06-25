## Usage

```sh
$ ./make-repo.sh
$ TOTAL=100 ./make-repo.sh
```

## What it does

Creates a git repository with a single commit and a large number of refs:

- lightweight tags: 50% of `TOTAL`
- annotated tags: 40% of `TOTAL`
- branches: 10% of `TOTAL`

All refs point to the single initial commit.

## Verify

```sh
$ git -C test-repo branch -l 'branch-*' | wc -l
$ git -C test-repo tag -l 'tag-*' | wc -l
$ git -C test-repo tag -l 'annotated-*' | wc -l
$ git -C test-repo rev-list --count HEAD
```
