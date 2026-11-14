.headers on
.mode list
.separator |
CREATE TABLE raw (id INTEGER PRIMARY KEY, a TEXT, b TEXT);
INSERT INTO raw (a, b) VALUES ('one', 'alpha'), ('two', 'beta'), ('three', 'gamma');
INSERT INTO raw (a, b) SELECT a || '-x', b || '-y' FROM raw;
DELETE FROM raw WHERE id % 2 = 0;
VACUUM;
SELECT id, a, b FROM raw ORDER BY id;
CREATE VIRTUAL TABLE boxes USING rtree(id, minx, maxx, miny, maxy);
INSERT INTO boxes VALUES
  (1, 0, 1, 0, 1),
  (2, 0.5, 1.5, 0.5, 1.5),
  (3, 10, 11, 10, 11),
  (4, -1, 0, -1, 0);
SELECT id FROM boxes WHERE minx <= 1 AND maxx >= 0.75 AND miny <= 1 AND maxy >= 0.75 ORDER BY id;
ATTACH ':memory:' AS mem;
CREATE TABLE mem.side (k INTEGER, v TEXT);
INSERT INTO mem.side VALUES (1, 'ok'), (2, 'yes');
SELECT k, v FROM mem.side ORDER BY k;
DETACH mem;
PRAGMA integrity_check;
