.headers on
.mode list
.separator |
CREATE TABLE orders (id INTEGER PRIMARY KEY, customer INTEGER, amount REAL, placed TEXT);
CREATE TABLE customers (id INTEGER PRIMARY KEY, name TEXT);
CREATE INDEX orders_customer ON orders(customer);
CREATE INDEX orders_amount ON orders(amount);
INSERT INTO customers VALUES (1, 'Ann'), (2, 'Bob'), (3, 'Cat');
INSERT INTO orders (customer, amount, placed) VALUES
  (1, 10.50, '2020-01-01'), (1, 4.25, '2020-01-03'),
  (2, 99.00, '2020-01-02'), (2, 1.00, '2020-01-04'),
  (3, 15.75, '2020-01-05'), (1, 20.00, '2020-02-01');
SELECT c.name, COUNT(o.id) AS n, printf('%.2f', SUM(o.amount)) AS total
  FROM customers c LEFT JOIN orders o ON o.customer = c.id
  GROUP BY c.id
  ORDER BY total+0 DESC, c.name;
SELECT c.name, o.amount
  FROM customers c
  JOIN orders o ON o.customer = c.id
  WHERE o.amount > 10
  ORDER BY o.amount, c.name;
EXPLAIN QUERY PLAN SELECT amount FROM orders WHERE customer = 1;
PRAGMA integrity_check;
