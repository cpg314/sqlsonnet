SELECT
  a,
  b,
  'c',
  d.*,
  sum(e + f) / 100 AS g
FROM (
  SELECT
    id
  FROM table1) AS table2
JOIN (
  SELECT
    *
  FROM table3) AS table4
  USING
    a,
    b
JOIN db1.table5 AS table6
  ON
    a = b
WHERE
  (a = b) AND TRUE
GROUP BY
  a + b AS c,
  d
HAVING
  a AND b
ORDER BY
  a ASC,
  b DESC
LIMIT 100
;
