WITH RECURSIVE depth_info AS (
    SELECT
        (SELECT depth FROM commits WHERE commit_id = 100) AS depth_a,
        (SELECT depth FROM commits WHERE commit_id = 20000) AS depth_b
),

delta_check AS (
    SELECT
        CASE
            WHEN depth_b < depth_a THEN 0
            WHEN
                depth_b = depth_a
                THEN (CASE WHEN 100 = 20000 THEN 1 END)
        END AS need_check
    FROM depth_info
),

recursive_jump AS (
    SELECT
        20000 AS current_commit,
        depth_b - depth_a AS delta,
        trunc(log2(depth_b - depth_a)) AS current_bit
    FROM depth_info
    WHERE depth_b > depth_a
    UNION ALL
    SELECT
        CASE
            WHEN
                ((delta >> current_bit) & 1) = 1
                THEN
                    (
                        SELECT ancestor_id
                        FROM ancestors
                        WHERE commit_id = current_commit AND level = current_bit
                    )
            ELSE current_commit
        END,
        delta,
        current_bit - 1
    FROM recursive_jump
    WHERE current_bit >= 0
)

-- select * from recursive_jump;
SELECT CASE
    WHEN (SELECT need_check FROM delta_check) = 0 THEN 0
    ELSE
        (SELECT current_commit = 100 FROM recursive_jump WHERE current_bit = -1)
END;
