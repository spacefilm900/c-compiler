-- Small classic benchmark kernels, written for this harness: binary trees,
-- n-body, spectral norm, fannkuch, mandelbrot. Fixed sizes, so the output is
-- fixed.
local function bottom_up(d)
  if d == 0 then return {} end
  d = d - 1
  return { bottom_up(d), bottom_up(d) }
end
local function check(t)
  if t[1] then return 1 + check(t[1]) + check(t[2]) end
  return 1
end
local maxd = 14
print("stretch", check(bottom_up(maxd + 1)))
local long = bottom_up(maxd)
for d = 4, maxd, 2 do
  local iters = 1 << (maxd - d + 4)
  local c = 0
  for _ = 1, iters do c = c + check(bottom_up(d)) end
  print("trees", iters, d, c)
end
print("long lived", check(long))

local PI, SOLAR = math.pi, 4 * math.pi * math.pi
local DAYS = 365.24
local bodies = {
  { 0, 0, 0, 0, 0, 0, SOLAR },
  { 4.84143144246472090e+00, -1.16032004402742839e+00, -1.03622044471123109e-01,
    1.66007664274403694e-03 * DAYS, 7.69901118419740425e-03 * DAYS, -6.90460016972063023e-05 * DAYS,
    9.54791938424326609e-04 * SOLAR },
  { 8.34336671824457987e+00, 4.12479856412430479e+00, -4.03523417114321381e-01,
    -2.76742510726862411e-03 * DAYS, 4.99852801234917238e-03 * DAYS, 2.30417297573763929e-05 * DAYS,
    2.85885980666130812e-04 * SOLAR },
  { 1.28943695621391310e+01, -1.51111514016986312e+01, -2.23307578892655734e-01,
    2.96460137564761618e-03 * DAYS, 2.37847173959480950e-03 * DAYS, -2.96589568540237556e-05 * DAYS,
    4.36624404335156298e-05 * SOLAR },
  { 1.53796971148509165e+01, -2.59193146099879641e+01, 1.79258772950371181e-01,
    2.68067772490389322e-03 * DAYS, 1.62824170038242295e-03 * DAYS, -9.51592254519715870e-05 * DAYS,
    5.15138902046611451e-05 * SOLAR },
}
local function energy()
  local e = 0
  for i = 1, #bodies do
    local b = bodies[i]
    e = e + 0.5 * b[7] * (b[4] * b[4] + b[5] * b[5] + b[6] * b[6])
    for j = i + 1, #bodies do
      local c = bodies[j]
      local dx, dy, dz = b[1] - c[1], b[2] - c[2], b[3] - c[3]
      e = e - b[7] * c[7] / math.sqrt(dx * dx + dy * dy + dz * dz)
    end
  end
  return e
end
local function advance(dt)
  local n = #bodies
  for i = 1, n do
    local b = bodies[i]
    for j = i + 1, n do
      local c = bodies[j]
      local dx, dy, dz = b[1] - c[1], b[2] - c[2], b[3] - c[3]
      local d2 = dx * dx + dy * dy + dz * dz
      local mag = dt / (d2 * math.sqrt(d2))
      local bm, cm = b[7] * mag, c[7] * mag
      b[4], b[5], b[6] = b[4] - dx * cm, b[5] - dy * cm, b[6] - dz * cm
      c[4], c[5], c[6] = c[4] + dx * bm, c[5] + dy * bm, c[6] + dz * bm
    end
  end
  for i = 1, n do
    local b = bodies[i]
    b[1], b[2], b[3] = b[1] + dt * b[4], b[2] + dt * b[5], b[3] + dt * b[6]
  end
end
local px, py, pz = 0, 0, 0
for _, b in ipairs(bodies) do px, py, pz = px + b[4] * b[7], py + b[5] * b[7], pz + b[6] * b[7] end
bodies[1][4], bodies[1][5], bodies[1][6] = -px / SOLAR, -py / SOLAR, -pz / SOLAR
print(string.format("nbody %.9f", energy()))
for _ = 1, 200000 do advance(0.01) end
print(string.format("nbody %.9f", energy()))

local function A(i, j) local ij = i + j - 1; return 1.0 / (ij * (ij - 1) * 0.5 + i) end
local function Av(x, y, N) for i = 1, N do local a = 0; for j = 1, N do a = a + x[j] * A(i, j) end; y[i] = a end end
local function Atv(x, y, N) for i = 1, N do local a = 0; for j = 1, N do a = a + x[j] * A(j, i) end; y[i] = a end end
local function AtAv(x, y, t, N) Av(x, t, N); Atv(t, y, N) end
local N = 300
local u, v, t = {}, {}, {}
for i = 1, N do u[i] = 1 end
for _ = 1, 10 do AtAv(u, v, t, N); AtAv(v, u, t, N) end
local vBv, vv = 0, 0
for i = 1, N do local ui, vi = u[i], v[i]; vBv = vBv + ui * vi; vv = vv + vi * vi end
print(string.format("spectral %0.9f", math.sqrt(vBv / vv)))

local function fannkuch(n)
  local p, q, s, sign, maxflips, sum = {}, {}, {}, 1, 0, 0
  for i = 1, n do p[i] = i; q[i] = i; s[i] = i end
  repeat
    local q1 = p[1]
    if q1 ~= 1 then
      for i = 2, n do q[i] = p[i] end
      local flips = 1
      while true do
        local qq = q[q1]
        if qq == 1 then
          sum = sum + sign * flips
          if flips > maxflips then maxflips = flips end
          break
        end
        q[q1] = q1
        if q1 >= 4 then
          local i, j = 2, q1 - 1
          repeat q[i], q[j] = q[j], q[i]; i = i + 1; j = j - 1 until i >= j
        end
        q1 = qq; flips = flips + 1
      end
    end
    if sign == 1 then
      p[2], p[1] = p[1], p[2]; sign = -1
    else
      p[2], p[3] = p[3], p[2]; sign = 1
      for i = 3, n do
        local sx = s[i]
        if sx ~= 1 then s[i] = sx - 1; break end
        if i == n then return sum, maxflips end
        s[i] = i
        local t = p[1]; for j = 1, i do p[j] = p[j + 1] end; p[i + 1] = t
      end
    end
  until false
end
print("fannkuch(9)", fannkuch(9))

local W, H, iter = 160, 160, 50
local inside = 0
for y = 0, H - 1 do
  local ci = 2.0 * y / H - 1.0
  for x = 0, W - 1 do
    local cr = 2.0 * x / W - 1.5
    local zr, zi, k = 0.0, 0.0, 0
    while k < iter and zr * zr + zi * zi <= 4.0 do
      zr, zi = zr * zr - zi * zi + cr, 2.0 * zr * zi + ci
      k = k + 1
    end
    if k == iter then inside = inside + 1 end
  end
end
print("mandelbrot inside", inside)
