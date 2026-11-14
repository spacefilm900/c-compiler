#include <stdio.h>
#include "abi.h"

int main(void)
{
    F1 f1 = mk_f1(1.5f);
    F4 f4 = mk_f4(2.0f);
    D2 d2 = mk_d2(3.0);
    D4 d4 = mk_d4(4.0);
    DA3 da3 = mk_da3(5.0);
    FD fd = mk_fd(6.5f, 7.25);
    C3 c3 = mk_c3(10);
    I2 i2 = mk_i2(11);
    I3 i3 = mk_i3(12);
    L2 l2 = mk_l2(13);
    L3 l3 = mk_l3(14);
    CF cf = mk_cf(15, 16.5f);
    Q q = mk_q(17);
    NF4 nf4 = mk_nf4(0.5f);
    printf("%g %g %g %g %g %g\n", f1.a, f4.a, f4.d, d2.b, d4.d, da3.a[2]);
    printf("%g %g %d %d %d %d %d %ld %ld %ld\n", fd.a, fd.b, c3.a, c3.c, i2.b, i3.c, cf.c, l2.b,
           l3.c, (long)(q.v >> 70));
    printf("%g %g\n", cf.f, nf4.f.d);
    printf("%g\n", take_hfa(f1, f4, d2, d4, da3, nf4));
    printf("%g\n", take_hfa_spill(d4, d4, mk_d4(1.0), 0.125, f4));
    printf("%ld\n", take_small(c3, i2, i3, l2, cf, fd));
    L3 a = mk_l3(1), b = mk_l3(2);
    printf("%ld %ld %ld\n", take_big(a, b, 3, mk_l3(4)), a.a, b.c);
    printf("%ld\n", take_q(1, q, ((__int128)9 << 64) | 8, 2));
    printf("%ld\n", take_narrow(-3, -4, 250, -300, 60000, 1, -7, 1L << 40, -9, -1000, 200, 1));
    printf("%g\n", take_floats(1.5f, 2.5, 3.5f, 4.5, 5.5f, 6.5, 7.5f, 8.5, 9.5f, 10.5, 11.5f));
    printf("%g\n", (double)take_ld(1.25L, 3, 0.5L));
    printf("%ld\n", sum_va(10, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10));
    printf("%g\n", sum_va_fp(10, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.5));
    printf("%ld\n", sum_va_mixed("icfDLqgsxld", 1, c3, f4, d4, l3, ((__int128)3 << 64) | 4,
                                 2.5L, i3, fd, 99L, 0.75));
    printf("%ld\n", sum_va_mixed("ddddddddddiiiiiiiiiiDf", 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0,
                                 8.0, 9.0, 10.0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, d4, f4));
    return 0;
}
