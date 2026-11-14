.headers on
.mode list
.separator |
PRAGMA foreign_keys = ON;
CREATE TABLE people (
  id INTEGER PRIMARY KEY,
  name TEXT NOT NULL,
  age INTEGER,
  city TEXT
);
CREATE TABLE cities (
  name TEXT PRIMARY KEY,
  country TEXT NOT NULL
);
INSERT INTO cities VALUES ('London', 'UK'), ('Paris', 'FR'), ('Tokyo', 'JP');
INSERT INTO people (name, age, city) VALUES
  ('Ada', 36, 'London'),
  ('Alan', 41, 'London'),
  ('Grace', 45, 'Paris'),
  ('Ken', 28, 'Tokyo'),
  ('Edsger', 52, 'Paris');
SELECT name, age FROM people WHERE age >= 40 ORDER BY age, name;
SELECT city, COUNT(*) AS n, AVG(age) AS avg_age FROM people GROUP BY city ORDER BY city;
SELECT p.name, c.country
  FROM people p JOIN cities c ON p.city = c.name
  ORDER BY c.country, p.name;
PRAGMA integrity_check;
