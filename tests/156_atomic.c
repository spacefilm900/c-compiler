/* Language-level _Atomic objects (C11 6.7.2.4, 6.7.3, 6.2.6.1p9, 6.5.16.2,
   6.5.2.4). Plain access is a seq_cst load or store; compound assignment
   and ++/-- are atomic read-modify-write. This file has no preprocessor
   directives, as required of tests/ from 119 onward. */

int printf(const char *, ...);

_Atomic int g;

int main(void)
{
    _Atomic int x;
    _Atomic unsigned u;
    int a[4];
    _Atomic(int *) p;
    int *old;
    _Atomic float f;
    x = 10;
    x += 3;
    ++x;
    x--;
    u = 1;
    u |= 2;
    u &= 3;
    g = 5;
    g += 1;
    a[0] = 1;
    a[1] = 2;
    a[2] = 3;
    a[3] = 4;
    p = a;
    old = p++;
    f = 1.5f;
    f += 2.25f;
    /* Lvalue conversion yields a non-atomic float; print that, not the
       atomic object, so a variadic call is not an atomic type. */
    {
        float fv = f;
        printf("%d %u %d %d %d %.2f\n", x, u, g, old[0], p[0], fv);
        return (x == 13 && u == 3 && g == 6 && old[0] == 1 && p[0] == 2 && fv > 3.7f && fv < 3.8f) ? 0 : 1;
    }
}
