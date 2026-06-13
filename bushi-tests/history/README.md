## Usage

```sh
$ ./make-repo.sh
$ TOTAL=100000 REPO_DIR=test-repo ./make-repo.sh
```

## Verify

```sh
$ git -C test-repo rev-list --count HEAD
$ git -C test-repo rev-list --count HEAD -- my.txt
$ git -C test-repo show HEAD:my.txt
```

