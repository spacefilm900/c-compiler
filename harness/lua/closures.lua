-- Closures, upvalues, recursion, varargs, tail calls, errors.
local function counter(start, step)
  local n = start
  return function() n = n + step; return n end
end
local c1, c2 = counter(0, 1), counter(100, -3)
local s = 0
for _ = 1, 100000 do s = s + c1() + c2() end
print("counters", s, c1(), c2())

local function fib(n) if n < 2 then return n end return fib(n - 1) + fib(n - 2) end
print("fib(27)", fib(27))

local function ack(m, n)
  if m == 0 then return n + 1 end
  if n == 0 then return ack(m - 1, 1) end
  return ack(m - 1, ack(m, n - 1))
end
print("ack(2,9)", ack(2, 9), "ack(3,5)", ack(3, 5))

local function loop(n, acc) if n == 0 then return acc end return loop(n - 1, acc + n) end
print("tail calls", loop(1000000, 0))

local function va(...)
  local n = select("#", ...)
  local sum = 0
  for i = 1, n do sum = sum + (select(i, ...) or 0) end
  return n, sum, ...
end
print("varargs", va(1, 2, nil, 4))
print("table of varargs", #{ va(5, 6, 7) })

local adders = {}
for i = 1, 10 do adders[i] = function(x) return x + i end end
local r = 0
for i = 1, 10 do r = r + adders[i](i) end
print("loop closures", r)

local function compose(f, g) return function(...) return f(g(...)) end end
local inc = function(x) return x + 1 end
local dbl = function(x) return x * 2 end
local f = inc
for _ = 1, 50 do f = compose(dbl, compose(inc, f)) end
print("composed", f(1) % 1000000007)

local ok, err = pcall(function() error({ code = 42 }) end)
print("pcall table", ok, err.code)
ok, err = pcall(function() error("boom", 0) end)
print("pcall string", ok, err)
ok, err = pcall(function() local x = nil; return x.y end)
print("pcall index", ok, (err:gsub("^.-:%d+: ", "")))
print("xpcall", xpcall(function() error("deep") end, function(m) return "handled: " .. m:gsub("^.-:%d+: ", "") end))
local depth = 0
local function rec() depth = depth + 1; return rec() + 1 end
ok = pcall(rec)
print("stack overflow caught", ok, depth > 1000)

local memo = setmetatable({}, { __mode = "k" })
local function memofib(n)
  if n < 2 then return n end
  local v = memo[n]
  if v then return v end
  v = memofib(n - 1) + memofib(n - 2)
  memo[n] = v
  return v
end
print("memo fib(90)", memofib(90))
