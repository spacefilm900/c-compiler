.headers on
.mode list
.separator |
CREATE TABLE t (i INTEGER PRIMARY KEY, x INTEGER, g INTEGER);
-- 8,000 rows from a fixed LCG (the same sequence every run).
WITH RECURSIVE r(i, x) AS (
  SELECT 1, 123456789
  UNION ALL
  SELECT i+1, (1103515245 * x + 12345) & 2147483647 FROM r WHERE i < 8000
)
INSERT INTO t(i, x, g) SELECT i, x, x % 17 FROM r;
CREATE INDEX t_g ON t(g);
SELECT COUNT(*) AS n, MIN(x) AS xmin, MAX(x) AS xmax, SUM(x) AS xsum FROM t;
SELECT g, COUNT(*) AS n, SUM(x) AS s FROM t GROUP BY g ORDER BY g;
SELECT COUNT(*) AS over FROM t WHERE x > 1000000000;
PRAGMA integrity_check;
