-- .eqp on
WITH RECURSIVE commit_tree AS (
    SELECT
        c.commit_id,
        c.commit_hash,
        c.repo_id,
        c.parent_id
    FROM commits AS c
    INNER JOIN refs ON c.commit_id = refs.commit_id
    WHERE
        c.commit_id = refs.commit_id
        AND c.repo_id = 1
        AND refs.short_name = 'master'

    UNION ALL

    SELECT
        c.commit_id,
        c.commit_hash,
        c.repo_id,
        c.parent_id
    FROM commits AS c
    INNER JOIN commit_tree AS ct ON c.commit_id = ct.parent_id
)

SELECT DISTINCT
    ct.commit_id,
    ct.commit_hash
FROM commit_tree AS ct
INNER JOIN commit_files AS cf ON ct.commit_id = cf.commit_id
INNER JOIN files AS f ON cf.file_id = f.file_id
WHERE f.name LIKE 'community/xmake%';

-- hyperfine 'sqlite3 < git-log-cte.sql .bushi.db > /dev/null'
-- 394ms
-- https://stackoverflow.com/questions/40329106/how-to-measure-the-execution-time-of-each-sql-statement-query-in-sqlite
