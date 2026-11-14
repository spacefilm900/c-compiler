/* Label addresses (GNU "Labels as Values") as address constants in static
   initializers: converted to a 64-bit integer, inside a union, and inside a
   union member of a struct array. Found while checking the Lua fix (LUA-1)
   beyond the plain table that Lua uses. GNU C only. */

int printf(const char *, ...);
union U { void *p; long l; };
struct S { int tag; union U u; };
int f(int k) {
    static long as_int = (long)&&b;
    static union U u = { &&a };
    static struct S s[2] = { {1, {.p = &&a}}, {2, {.p = &&b}} };
    void *t = k ? (void *)as_int : u.p;
    if (s[k].u.p != t) return -1;
    goto *t;
a:  return 10 + s[0].tag;
b:  return 20 + s[1].tag;
}
int main(void) { printf("%d %d\n", f(0), f(1)); return 0; }
