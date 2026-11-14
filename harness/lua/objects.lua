-- Metatables and object-oriented code: inheritance, operators, __index
-- chains, __newindex, __call, __concat, __len, __close.
local Vec = {}
Vec.__index = Vec
function Vec.new(x, y) return setmetatable({ x = x, y = y }, Vec) end
function Vec.__add(a, b) return Vec.new(a.x + b.x, a.y + b.y) end
function Vec.__sub(a, b) return Vec.new(a.x - b.x, a.y - b.y) end
function Vec.__mul(a, k) return Vec.new(a.x * k, a.y * k) end
function Vec.__eq(a, b) return a.x == b.x and a.y == b.y end
function Vec.__lt(a, b) return a:len2() < b:len2() end
function Vec.__le(a, b) return a:len2() <= b:len2() end
function Vec.__tostring(a) return "(" .. a.x .. "," .. a.y .. ")" end
function Vec.__concat(a, b) return tostring(a) .. tostring(b) end
function Vec.__unm(a) return Vec.new(-a.x, -a.y) end
function Vec.__len(a) return 2 end
function Vec:len2() return self.x * self.x + self.y * self.y end

local acc = Vec.new(0, 0)
for i = 1, 200000 do acc = acc + Vec.new(i % 7, -(i % 5)) * 2 - Vec.new(1, 1) end
print("vector sum", tostring(acc), #acc, acc == Vec.new(acc.x, acc.y), Vec.new(1, 1) < Vec.new(2, 0))
print("concat", Vec.new(1, 2) .. Vec.new(3, 4), tostring(-Vec.new(5, 6)))

local Animal = {}
Animal.__index = Animal
function Animal.new(name, sound) local o = setmetatable({}, Animal); o.name = name; o.sound = sound; return o end
function Animal:speak() return self.name .. " says " .. self.sound end
local Dog = setmetatable({}, { __index = Animal })
Dog.__index = Dog
function Dog.new(name) local o = Animal.new(name, "woof"); return setmetatable(o, Dog) end
function Dog:fetch() return self.name .. " fetches" end
local d = Dog.new("rex")
print(d:speak(), d:fetch(), getmetatable(d) == Dog, rawget(d, "speak"))

local log = {}
local proxy = setmetatable({}, {
  __index = function(_, k) log[#log + 1] = "get " .. k; return k:upper() end,
  __newindex = function(t, k, v) log[#log + 1] = "set " .. k; rawset(t, k, v * 10) end,
  __call = function(self, a, b) return a * b end,
})
print(proxy.abc, proxy(6, 7))
proxy.z = 5
print(proxy.z, table.concat(log, ";"))

local calls = 0
local deep = {}
local cur = deep
for i = 1, 50 do
  local nxt = {}
  setmetatable(cur, { __index = nxt })
  cur = nxt
end
cur.leaf = "found"
for _ = 1, 20000 do if deep.leaf then calls = calls + 1 end end
print("index chain", calls, deep.leaf)

do
  local order = {}
  do
    local a <close> = setmetatable({}, { __close = function() order[#order + 1] = "a" end })
    local b <close> = setmetatable({}, { __close = function() order[#order + 1] = "b" end })
  end
  print("close order", table.concat(order, ","))
end

local Stack = {}
Stack.__index = Stack
function Stack.new() return setmetatable({ n = 0 }, Stack) end
function Stack:push(v) self.n = self.n + 1; self[self.n] = v end
function Stack:pop() local v = self[self.n]; self[self.n] = nil; self.n = self.n - 1; return v end
local st = Stack.new()
local checksum = 0
for i = 1, 300000 do
  st:push(i)
  if i % 3 == 0 then checksum = checksum + st:pop() end
end
print("stack", st.n, checksum)
