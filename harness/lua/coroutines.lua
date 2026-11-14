-- Coroutines: generators, pipelines, wrap, status, yields across pcall.
local function gen(n)
  return coroutine.wrap(function()
    for i = 1, n do coroutine.yield(i) end
  end)
end

local function filter(pred, src)
  return coroutine.wrap(function()
    for v in src do if pred(v) then coroutine.yield(v) end end
  end)
end

local function map(fn, src)
  return coroutine.wrap(function()
    for v in src do coroutine.yield(fn(v)) end
  end)
end

local sum, count = 0, 0
for v in map(function(x) return x * x end, filter(function(x) return x % 3 == 0 end, gen(300000))) do
  sum = sum + v
  count = count + 1
end
print("pipeline", count, sum)

local co = coroutine.create(function(a, b)
  local c = coroutine.yield(a + b)
  local d, e = coroutine.yield(c * 2)
  return d + e, "done"
end)
print(coroutine.resume(co, 1, 2))
print(coroutine.status(co))
print(coroutine.resume(co, 10))
print(coroutine.resume(co, 3, 4))
print(coroutine.status(co), coroutine.resume(co))

local prod = coroutine.create(function()
  for i = 1, 5 do
    local ok, v = pcall(function()
      if i == 3 then error("bad item") end
      return coroutine.yield(i)
    end)
    if not ok then coroutine.yield("err") end
  end
  return "end"
end)
local got = {}
while true do
  local ok, v = coroutine.resume(prod, "ack")
  got[#got + 1] = tostring(v)
  if coroutine.status(prod) == "dead" then break end
end
print("yield across pcall", table.concat(got, ","))

-- Many live coroutines, switched round robin.
local cos = {}
for i = 1, 500 do
  cos[i] = coroutine.create(function()
    local acc = 0
    for j = 1, 200 do acc = acc + i * j; coroutine.yield() end
    return acc
  end)
end
local total = 0
for _ = 1, 201 do
  for i = 1, #cos do
    local ok, r = coroutine.resume(cos[i])
    if r then total = total + r end
  end
end
print("round robin", total)

print("isyieldable main", coroutine.isyieldable())
print("running main", select(2, coroutine.running()))
local w = coroutine.wrap(function() error("inside") end)
local ok, err = pcall(w)
print("error in wrap", ok, (err:gsub("^.-:%d+: ", "")))
print("close", coroutine.close(coroutine.create(function() end)))
