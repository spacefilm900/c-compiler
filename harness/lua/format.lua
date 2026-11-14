-- Formatting and parsing of floating-point numbers.
local values = {
  0.0, -0.0, 1.0, -1.5, 0.1, 1 / 3, 2 / 3, 1e15, 1e16, 1e100, 1e-5, 1e-300, 5e-324,
  2.2250738585072014e-308, 1.7976931348623157e308, 123456789.125, 3.141592653589793,
  2^53, 2^63, -2^63, 0.5, 100, 255.255, 1e21, 123e-20,
}
local fmts = { "%g", "%.3g", "%.17g", "%e", "%.0e", "%.12e", "%f", "%.2f", "%10.4f", "%-12.3e|", "%+.5g", "%a", "%.3a", "%5.1f" }
for _, v in ipairs(values) do
  local out = {}
  for _, f in ipairs(fmts) do
    if not (f:find("f") and math.abs(v) > 1e30) then out[#out + 1] = string.format(f, v) end
  end
  print(table.concat(out, " "))
end
for _, v in ipairs(values) do io.write(tostring(v), " ", string.format("%q", v), "\n") end
print(1 / 0, -1 / 0, string.format("%f %g %e", 1 / 0, -1 / 0, 1 / 0))

local strings = {
  "0", "1e1", "  12  ", "0x1p4", "0x.8", "0X1P-2", "1e400", "-1e400", ".5", "5.", "1e-400",
  "0x7fffffffffffffff", "0xffffffffffffffff", "9223372036854775807", "9223372036854775808",
  "-9223372036854775808", "1e", "abc", "0x", "3.0", "  -0x10  ", "1 2", "0.1e+2",
}
for _, s in ipairs(strings) do
  local n = tonumber(s)
  print(string.format("%-24s", string.format("%q", s)), math.type(n), n)
end
print(tonumber("ff", 16), tonumber("zz", 36), tonumber("777", 8), tonumber("101", 2), tonumber("8", 8))

local sum = 0
for i = 1, 200000 do
  local s = string.format("%.14g", i / 7)
  sum = sum + tonumber(s)
  if i % 50000 == 0 then print(s, string.format("%.3f", tonumber(s) * 1e6)) end
end
print(string.format("roundtrip sum %.6f", sum))
local parts = {}
for i = -20, 20, 3 do parts[#parts + 1] = tostring(10.0 ^ i) end
print(table.concat(parts, " "))
print(string.format("%5d|%-5d|%+d|% d|%i", 42, 42, 42, 42, -7))
print(math.pi, -math.pi, 100 / 3, 2^-1074, 2^1023 * 1.5)
