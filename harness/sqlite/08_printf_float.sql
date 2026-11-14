.headers on
.mode list
.separator |
CREATE TABLE nums (x REAL);
INSERT INTO nums VALUES (0), (1), (-1), (0.1), (1.0/3), (2.5), (1e16), (3.141592653589793);
SELECT x,
       printf('%.1f', x) AS f1,
       printf('%.6f', x) AS f6,
       printf('%g', x) AS g,
       printf('%e', x) AS e,
       round(x, 4) AS r4,
       abs(x) AS ax
  FROM nums
  ORDER BY rowid;
SELECT printf('%d %s %Q', 42, 'hello', 'o''reilly') AS s;
SELECT printf('%02d:%02d', 7, 5) AS clock;
SELECT sin(0), cos(0), pow(2, 10), ln(1), sqrt(9), floor(2.75), ceil(-1.25);
PRAGMA integrity_check;
