# Example

## Prepare

```
$ ./history/make-repo
$ bushi-index -t test.db -a history/test-repo/
$ bushi-index -t test.db test-repo
```

## Check

```sh
$ ./demo-cli.py -t test.db test-repo -- my.txt
```

```sh
$ git -C history/test-repo log --first-parent --format=%H main -- my.txt
```

