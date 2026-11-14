.headers on
.mode list
.separator |
CREATE TABLE sales (day INTEGER, shop TEXT, qty INTEGER);
INSERT INTO sales VALUES
  (1, 'A', 10), (1, 'B', 7), (2, 'A', 3), (2, 'B', 12),
  (3, 'A', 8), (3, 'B', 5), (4, 'A', 0), (4, 'B', 9);
SELECT day, shop, qty,
       SUM(qty) OVER (PARTITION BY shop ORDER BY day) AS running,
       RANK() OVER (PARTITION BY day ORDER BY qty DESC) AS rnk,
       LAG(qty, 1, 0) OVER (PARTITION BY shop ORDER BY day) AS prev,
       AVG(qty) OVER (ORDER BY day ROWS BETWEEN 1 PRECEDING AND 1 FOLLOWING) AS mov
  FROM sales
  ORDER BY shop, day;
PRAGMA integrity_check;
