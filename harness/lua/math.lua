-- Integer and float arithmetic, overflow, conversions, math library.
local mi, ma = math.mininteger, math.maxinteger
print("limits", mi, ma, ma + 1 == mi, mi - 1 == ma, -mi == mi)
print("int ops", 7 // 2, -7 // 2, 7 // -2, 7 % -3, -7 % 3, 7.5 // 2, -7.5 % 2)
print("int div edge", mi // -1, mi % -1, pcall(function() return 1 // 0 end))
print("float div", 1 / 0, -1 / 0, 0 / 0 ~= 0 / 0, 1 // 0.0, -1 % math.huge)
print("bitwise", 0xF0 & 0x3C, 0xF0 | 0x0F, 0xFF ~ 0x0F, ~0, 1 << 63, 1 << 64, -1 >> 1, 3 << -1)
print("shifts", 0x123456789 >> 4, 0x123456789 << 20, (-8) >> 60)
print("conversions", math.tointeger(3.0), math.tointeger(3.5), 3 | 0, math.type(1), math.type(1.0), math.type("1"))
print("floor/ceil", math.floor(-3.5), math.ceil(-3.5), math.floor(2^53 + 0.0), math.ceil(1e300) == 1e300)
print("fmod", math.fmod(7, 3), math.fmod(-7, 3), math.fmod(7, -3), math.fmod(-7.5, 2), math.fmod(mi, -1))
print("abs", math.abs(mi), math.abs(-0.0), math.abs(-3))
print("ult", math.ult(1, -1), math.ult(-1, 1))
print("max/min", math.max(1, 2.5, -3), math.min(4, 2, 8.0), math.max(mi, ma))
print("string arith", "10" + 5, "3.5" * 2, 10 .. "", 2^10, "0x10" + 0)
print("compare", 1 == 1.0, 2^53 == 2^53 + 1, ma + 0.0 == ma, mi + 0.0 == mi, 1 < 1.5, -0.0 == 0.0)

local s = 0.0
for i = 1, 2000000 do s = s + math.sin(i * 0.001) * math.cos(i * 0.002) end
print(string.format("trig sum %.10f", s))
local e = 0
for i = 1, 300000 do e = e + math.sqrt(i) + math.log(i) + math.exp(-i / 1e5) end
print(string.format("sqrt log exp %.6f", e))
print(string.format("atan %.15g %.15g %.15g", math.atan(1), math.atan(1, -1), math.atan(-0.0, -1)))
print(string.format("log %.15g %.15g %.15g", math.log(8, 2), math.log(1000, 10), math.log(2.0)))

local acc = 0
for i = 1, 3000000 do acc = (acc * 6364136223846793005 + 1442695040888963407 + i) end
print("lcg wrap", acc)

local h = 0
for i = 1, 1000000 do h = h ~ ((i * 0x9E3779B97F4A7C15) >> 17) ; h = (h << 5) | (h >> 59) end
print("bit mix", h)

math.randomseed(42, 7)
local r = {}
for i = 1, 8 do r[i] = math.random(1, 1000) end
print("random ints", table.concat(r, ","))
print(string.format("random floats %.17g %.17g", math.random(), math.random()))
print("random full", math.random(0))

-- Collatz and primes: integer loops.
local best, bestn = 0, 0
for n = 1, 300000 do
  local x, steps = n, 0
  while x ~= 1 do
    if x % 2 == 0 then x = x // 2 else x = 3 * x + 1 end
    steps = steps + 1
  end
  if steps > best then best, bestn = steps, n end
end
print("collatz", bestn, best)
local sieve, primes = {}, 0
for i = 2, 2000000 do
  if not sieve[i] then
    primes = primes + 1
    for j = i * i, 2000000, i do sieve[j] = true end
  end
end
print("primes below 2e6", primes)
