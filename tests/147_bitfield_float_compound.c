/* Compound assignment with a floating operand to a bit-field writes only
   that bit-field (C11 6.5.16.2p3, 6.7.2.1p10). Before the fix the store
   used the whole storage unit and cleared the neighbouring bit-fields.
   The result also converts straight to the bit-field's declared type, so
   `_Bool f : 1; f += 0.25` is 1 and a `long long` right operand is not
   cut to `int` first. */
int printf(const char *, ...);

struct U { unsigned a : 4, b : 4, c : 8; };
struct S { int x : 5; int y : 7; signed char z : 3; };
struct L { unsigned long long lo : 20, mid : 30, hi : 14; };
struct B { _Bool f : 1; unsigned rest : 7; };
struct W { _Bool f : 1; unsigned rest : 7; signed char c : 5; unsigned long long w : 40; };

int main(void) {
    struct U u = { 1, 7, 200 };
    int r1 = (u.a += 1.0);
    u.b -= 2.0f;
    u.a *= 2.5;
    u.c /= 4.0L;
    printf("%u %u %u %d\n", u.a, u.b, u.c, r1);

    struct S s = { -3, 40, -2 };
    s.x += 1.5;
    s.y -= 50.25;
    s.z *= 1.0;
    int r2 = (s.x -= 2.0);
    printf("%d %d %d %d\n", s.x, s.y, s.z, r2);

    struct L l = { 0xfffff, 12345, 0x3fff };
    l.mid += 0.5e3;
    l.lo /= 2.0;
    printf("%llu %llu %llu\n", l.lo, l.mid, l.hi);

    struct B b = { 0, 0x55 };
    b.f += 0.25;
    printf("%d %u\n", b.f, b.rest);
    b.f -= 1.0;
    printf("%d %u\n", b.f, b.rest);

    struct W w = { 0, 3, 2, 5 };
    w.f += 0x100000000LL;
    int r3 = (w.f += 0.5);
    w.c += 0x100000003LL;
    w.w += 0x10000000001ULL;
    w.rest |= 0x80000001LL;
    printf("%d %d %d %llu %u\n", w.f, r3, w.c, (unsigned long long)w.w, w.rest);

    double d = 3.0;
    u.b = 1;
    u.b += d;
    printf("%u %u %u\n", u.a, u.b, u.c);
    return u.b == 4 && u.c == 50 ? 0 : 1;
}
