.headers on
.mode list
.separator |
CREATE VIRTUAL TABLE docs USING fts5(title, body, tokenize = 'porter');
INSERT INTO docs (title, body) VALUES
  ('Compiler notes', 'A C compiler emits LLVM IR and links with Clang.'),
  ('Database intro', 'SQLite is a small SQL database engine used everywhere.'),
  ('Query planner', 'Indexes and joins are chosen by the SQLite query planner.'),
  ('Full text', 'Full-text search finds documents that mention a compiler or a database.');
SELECT title FROM docs WHERE docs MATCH 'compiler' ORDER BY title;
SELECT title FROM docs WHERE docs MATCH 'sqlite OR database' ORDER BY title;
SELECT title, highlight(docs, 1, '[', ']') AS snip
  FROM docs WHERE docs MATCH 'query' ORDER BY title;
SELECT COUNT(*) AS n FROM docs;
PRAGMA integrity_check;
