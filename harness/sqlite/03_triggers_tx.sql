.headers on
.mode list
.separator |
CREATE TABLE accounts (id INTEGER PRIMARY KEY, name TEXT, balance INTEGER);
CREATE TABLE ledger (id INTEGER PRIMARY KEY, account INTEGER, delta INTEGER, note TEXT);
CREATE TRIGGER trg_balance AFTER UPDATE OF balance ON accounts
BEGIN
  INSERT INTO ledger(account, delta, note)
    VALUES (NEW.id, NEW.balance - OLD.balance, 'trigger');
END;
INSERT INTO accounts VALUES (1, 'alice', 100), (2, 'bob', 50);
BEGIN;
UPDATE accounts SET balance = balance - 30 WHERE id = 1;
UPDATE accounts SET balance = balance + 30 WHERE id = 2;
SELECT * FROM accounts ORDER BY id;
SELECT account, delta FROM ledger ORDER BY id;
COMMIT;
BEGIN;
UPDATE accounts SET balance = balance - 1000 WHERE id = 1;
SELECT balance FROM accounts WHERE id = 1;
ROLLBACK;
SELECT * FROM accounts ORDER BY id;
SELECT COUNT(*) AS ledger_rows FROM ledger;
PRAGMA integrity_check;
