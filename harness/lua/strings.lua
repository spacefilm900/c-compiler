-- String operations: building, slicing, patterns, gsub, packing.
-- Deterministic: prints only values that do not depend on addresses or
-- on table iteration order.
local function checksum(s)
  local h = 0
  for i = 1, #s do h = (h * 31 + s:byte(i)) % 4294967296 end
  return h
end

local words = {}
for i = 1, 2000 do
  words[#words + 1] = string.format("w%04d_%s", i, string.rep(string.char(97 + i % 26), i % 7 + 1))
end
local text = table.concat(words, " ")
print("text length", #text, "checksum", checksum(text))

local total = 0
for round = 1, 60 do
  local up = text:upper()
  local swapped = up:gsub("W(%d+)_(%a+)", "%2-%1")
  local n = 0
  for num in swapped:gmatch("%-(%d+)") do n = n + tonumber(num) end
  total = total + n + #swapped
  if round == 1 then
    print("first swap", swapped:sub(1, 60))
    print("sum of numbers", n)
  end
end
print("total", total)

local found = {}
for _, pat in ipairs({ "w0042_%a+", "(%d%d)99", "z+", "^w0001", "%s(w1%d%d%d)_", "[aeiou]+$" }) do
  local s, e, cap = text:find(pat)
  found[#found + 1] = string.format("%s:%s:%s:%s", pat, tostring(s), tostring(e), tostring(cap))
end
print(table.concat(found, " | "))

local rev = {}
for i = 1, 300 do rev[i] = words[i]:reverse() end
print("reversed", checksum(table.concat(rev)))

local bytes = {}
for i = 0, 255 do bytes[#bytes + 1] = string.char(i) end
local all = table.concat(bytes)
print("byte range", #all, all:byte(1), all:byte(-1), checksum(all:lower()), checksum(all:upper()))

local packed = string.pack("<i4 >i8 d z s2 B", -123456, 0x1122334455667788, 3.25, "zero", "len", 200)
print("pack", #packed, checksum(packed))
print("unpack", string.unpack("<i4 >i8 d z s2 B", packed))

local parts = {}
for k, v in string.gmatch("a=1, bb=22, ccc=333, dddd=4444", "(%w+)=(%w+)") do
  parts[#parts + 1] = k .. ":" .. (tonumber(v) * 2)
end
print(table.concat(parts, ","))
print(("%5s|%-5s|%05d|%x|%X|%o|%c"):format("ab", "cd", 42, 255, 48879, 8, 65))
print(string.format("%q", "line1\nline2\0tab\t\"quoted\"\\"))
print(("hello world from lua"):gsub("(%w+)", "<%1>"))
print(("abc"):rep(5, "-"), ("x"):rep(0), #("ab"):rep(1000))
print(utf8.char(72, 228, 8364, 128512), utf8.len("häll€"), utf8.codepoint("€", 1))
