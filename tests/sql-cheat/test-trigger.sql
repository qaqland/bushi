.eqp full
    INSERT INTO
        ancestors (commit_id, level, ancestor_id)
    WITH RECURSIVE skip_cte (commit_id, level, ancestor_id) AS (
    SELECT
        commit_id,
        0 AS level,
        (SELECT
            commit_id
        FROM
            commits
        WHERE
            repo_id = 1
            AND commit_mark = 19999
        ) AS ancestor_id
    FROM
        commits
    WHERE
        commit_id = 20000

    UNION ALL

    SELECT
        s.commit_id, s.level + 1, a.ancestor_id
    FROM
        skip_cte AS s
    INNER JOIN
        ancestors AS a
    ON
        s.ancestor_id = a.commit_id
        AND s.level = a.level
    )

    SELECT
        commit_id, level, ancestor_id
    FROM
        skip_cte
    WHERE
        ancestor_id IS NOT NULL;