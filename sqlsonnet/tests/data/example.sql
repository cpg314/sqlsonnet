SELECT
  1,
  1,
  true,
  "string",
  col,
  col AS alias,
  1 + 2,
  1 = 2,
  count(*),
  NOT (1 != 2),
  text LIKE '%t%'
FROM a
JOIN b
  ON
    f1=f2
JOIN c
  USING
    f
CROSS JOIN d
LEFT OUTER JOIN e
  USING
    f
WHERE
  true
HAVING
  true
ORDER BY
  col1,
  col2 DESC,
  col3
LIMIT 100
SAMPLE 100
SETTINGS join_algorithm="parallel_hash"
;
SELECT
  0,
  1
FROM a
JOIN b
  USING
    col1
JOIN c
  USING
    col2
;
SELECT
  0
FROM a
WHERE
  (2 >= 1)
  AND (1 = 1)
;
SELECT
  0
LIMIT 10 BY
  id,
  debits - credits
;
