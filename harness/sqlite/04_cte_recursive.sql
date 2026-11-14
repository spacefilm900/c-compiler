.headers on
.mode list
.separator |
CREATE TABLE edge (src INTEGER, dst INTEGER);
INSERT INTO edge VALUES (1,2), (1,3), (2,4), (3,4), (4,5);
WITH RECURSIVE
  walk(n, path, depth) AS (
    SELECT 1, '1', 0
    UNION ALL
    SELECT e.dst, walk.path || '->' || e.dst, walk.depth + 1
      FROM walk JOIN edge e ON e.src = walk.n
     WHERE walk.depth < 4
  )
SELECT n, path, depth FROM walk ORDER BY depth, path;
WITH RECURSIVE fib(i, a, b) AS (
  SELECT 1, 0, 1
  UNION ALL
  SELECT i+1, b, a+b FROM fib WHERE i < 20
)
SELECT i, a FROM fib ORDER BY i;
PRAGMA integrity_check;
