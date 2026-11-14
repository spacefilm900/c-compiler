/* Label addresses (GNU "Labels as Values") as address constants in static
   initializers: converted to a 64-bit integer, inside a union, and inside a
   union member of a struct array. Found while checking the Lua fix (LUA-1)
   beyond the plain table that Lua uses. GNU C only. Each table is checked
   by jumping through it, never by comparing two label addresses, which the
   LLVM Language Reference leaves undefined. */

int printf(const char *, ...);
union U { void *p; long l; };
struct S { int tag; union U u; };
int f(int k) {
    static long as_int = (long)&&b;
    static union U u = { &&a };
    static struct S s[2] = { {1, {.p = &&a}}, {2, {.p = &&b}} };
    int r = 0;
    void *t = k ? (void *)as_int : u.p;
    goto *t;
a:  r += 10;
    goto *s[1].u.p;
b:  if (r == 0 && k == 1) {
        r = 1;
        goto *s[0].u.p;
    }
    return r + 20 + s[k].tag;
}
int main(void) { printf("%d %d\n", f(0), f(1)); return 0; }
