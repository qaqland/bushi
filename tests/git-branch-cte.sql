-- .eqp on
WITH RECURSIVE commit_chain AS (
    SELECT
        r.full_name,
        c.commit_id,
        c.commit_hash,
        c.parent_id
    FROM
        refs AS r
    INNER JOIN
        commits AS c
        ON r.commit_id = c.commit_id

    UNION ALL

    SELECT
        cc.full_name,
        p.commit_id,
        p.commit_hash,
        p.parent_id
    FROM
        commit_chain AS cc
    INNER JOIN
        commits AS p
        ON cc.parent_id = p.commit_id
    WHERE
        cc.commit_hash <> 'c1893546b4ffd9ad69a5710ef58917849f278c11'
)

SELECT DISTINCT full_name
FROM
    commit_chain
WHERE
    commit_hash = 'c1893546b4ffd9ad69a5710ef58917849f278c11';

-- cte ~28s

-- git ~0.25s
-- GIT_PAGER=cat time git tag/branch \
--	--contains c1893546b4ffd9ad69a5710ef58917849f278c11 \
--	--format '%(refname)'
