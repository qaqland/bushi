# Limits in Bushi

## Max Number Of Changed Files In A Commit

32766 (maybe smaller)

see `SQLITE_LIMIT_VARIABLE_NUMBER` interface

## Slash In Reference Name

git branch or tag named like `fix/typos` would be converted to

<http://example.com/repo/-/blob/head/fix:typos/README.md>

References:

- <https://stackoverflow.com/questions/1737575/are-colons-allowed-in-urls>
- <http://www.vimeo.com/tag:sample>
- <https://en.wikipedia.org/wiki/Template:Welcome>

