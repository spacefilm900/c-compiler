-- Tables: arrays, hashes, insert/remove, sort with comparators, length.
local N = 200000
local t = {}
for i = 1, N do t[i] = (i * 7919) % 100003 end
local s = 0
for i = 1, #t do s = s + t[i] end
print("array sum", #t, s)

table.sort(t)
print("sorted", t[1], t[2], t[N // 2], t[N])
table.sort(t, function(a, b) return a > b end)
print("desc", t[1], t[2], t[N])

local h = {}
for i = 1, 100000 do h["k" .. (i % 5003)] = (h["k" .. (i % 5003)] or 0) + i end
local keys = {}
for k in pairs(h) do keys[#keys + 1] = k end
table.sort(keys)
local acc = 0
for i, k in ipairs(keys) do acc = (acc + i * h[k]) % 1000000007 end
print("hash keys", #keys, keys[1], keys[#keys], acc)

local q = {}
for i = 1, 20000 do table.insert(q, i) end
for i = 1, 5000 do table.insert(q, 1, -i) end
local removed = 0
for i = 1, 7000 do removed = removed + table.remove(q, 1) end
for i = 1, 3000 do removed = removed + table.remove(q) end
print("queue", #q, q[1], q[#q], removed)

local moved = table.move({ 1, 2, 3, 4, 5 }, 2, 4, 1, { 9, 9, 9, 9, 9, 9 })
print("move", table.concat(moved, ","))
print("unpack", table.unpack({ 10, 20, 30, nil, 50 }, 1, 5))
print("pack n", table.pack(1, nil, 3).n)

local mixed = { 1, 2, 3, x = 1, y = 2, [10] = 10, [-1] = "neg" }
print("mixed", mixed[3], #mixed >= 3, mixed[-1], mixed[10], mixed.x + mixed.y)

local matrix = {}
for i = 1, 120 do
  matrix[i] = {}
  for j = 1, 120 do matrix[i][j] = (i * j) % 17 end
end
local prod = {}
for i = 1, 120 do
  local row = {}
  for j = 1, 120 do
    local sum = 0
    for k = 1, 120 do sum = sum + matrix[i][k] * matrix[k][j] end
    row[j] = sum
  end
  prod[i] = row
end
local trace = 0
for i = 1, 120 do trace = trace + prod[i][i] end
print("matrix trace", trace, prod[7][11], prod[120][1])

-- Records sorted by several fields; ties broken by id so the order is total.
local recs = {}
for i = 1, 30000 do recs[i] = { id = i, a = i % 13, b = (i * 37) % 101 } end
table.sort(recs, function(x, y)
  if x.a ~= y.a then return x.a < y.a end
  if x.b ~= y.b then return x.b > y.b end
  return x.id < y.id
end)
print("records", recs[1].id, recs[2].id, recs[15000].id, recs[30000].id)
