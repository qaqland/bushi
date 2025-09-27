# Offline Git Repositories

```bash
git clone --mirror https://github.com/alpinelinux/apk-tools.git
cd apk-tools
```

```bash
git bundle create apk-tools.bundle --all
```

```bash
$ du -sh apk-tools.bundle
4.1M    apk-tools.bundle
```

```bash
git clone --bare apk-tools.bundle <new directory>
```

