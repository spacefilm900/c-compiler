.headers on
.mode list
.separator |
CREATE TABLE ev (id INTEGER PRIMARY KEY, payload TEXT);
INSERT INTO ev(payload) VALUES
  (json_object('user', 'ada', 'score', 10, 'tags', json_array('a','b'))),
  (json_object('user', 'alan', 'score', 7, 'tags', json_array('b','c'))),
  (json_object('user', 'grace', 'score', 15, 'tags', json_array('a')));
SELECT json_extract(payload, '$.user') AS user,
       json_extract(payload, '$.score') AS score
  FROM ev
  WHERE json_extract(payload, '$.score') >= 10
  ORDER BY score, user;
SELECT e.id, t.value AS tag
  FROM ev e, json_each(e.payload, '$.tags') AS t
  ORDER BY e.id, t.value;
SELECT json_group_array(json_extract(payload, '$.user')) AS users FROM ev;
SELECT json_valid(payload), json_type(payload, '$.tags') FROM ev ORDER BY id;
PRAGMA integrity_check;
