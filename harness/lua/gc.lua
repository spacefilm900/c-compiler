-- Garbage collector stress: allocation churn, weak tables, finalizers,
-- generational and incremental modes. Prints only deterministic values.
local function churn(n)
  local keep = {}
  for i = 1, n do
    local t = { i, tostring(i), { i * 2 } }
    if i % 100 == 0 then keep[#keep + 1] = t end
  end
  local s = 0
  for _, t in ipairs(keep) do s = s + t[1] + #t[2] + t[3][1] end
  return #keep, s
end

print("incremental", collectgarbage("incremental"))
print("churn", churn(300000))
collectgarbage("generational")
print("churn gen", churn(300000))
collectgarbage("incremental")

local weak = setmetatable({}, { __mode = "v" })
local strong = {}
for i = 1, 1000 do
  local obj = { id = i }
  weak[i] = obj
  if i % 10 == 0 then strong[#strong + 1] = obj end
end
collectgarbage()
collectgarbage()
local alive = 0
for i = 1, 1000 do if weak[i] then alive = alive + 1 end end
print("weak values alive", alive)

local wk = setmetatable({}, { __mode = "k" })
for i = 1, 500 do wk[{}] = i end
local kept = {}
for i = 1, 50 do local k = {}; kept[i] = k; wk[k] = i end
collectgarbage()
local n = 0
for _ in pairs(wk) do n = n + 1 end
print("weak keys alive", n)

local order = {}
do
  for i = 1, 20 do
    setmetatable({}, { __gc = function() order[#order + 1] = i end })
  end
end
collectgarbage()
collectgarbage()
print("finalizers", #order, table.concat(order, ","))

local resurrected
do
  setmetatable({ name = "phoenix" }, { __gc = function(o) resurrected = o end })
end
collectgarbage()
print("resurrected", resurrected and resurrected.name)

-- Strings: many short and long ones, interned and not.
local strs = {}
for i = 1, 50000 do strs[i] = ("s" .. i):rep(i % 5 + 1) end
local len = 0
for i = 1, #strs, 7 do len = len + #strs[i] end
strs = nil
collectgarbage()
print("strings", len)

-- Deep structure freed at once.
local function tree(d) if d == 0 then return {} end return { tree(d - 1), tree(d - 1) } end
local function count(t) if not t[1] then return 1 end return 1 + count(t[1]) + count(t[2]) end
for d = 10, 16, 3 do
  local t = tree(d)
  print("tree", d, count(t))
end
collectgarbage("step", 0)
print("isrunning", collectgarbage("isrunning"))
collectgarbage("stop")
print("stopped", collectgarbage("isrunning"))
collectgarbage("restart")
print("done")
