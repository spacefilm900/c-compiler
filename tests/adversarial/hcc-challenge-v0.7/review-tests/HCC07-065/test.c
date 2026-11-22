/* HCC07-065: Thread feature-test macros and independent block scope TLS
 * Basis: 6.10.8.3; 6.2.4p4; 7.26
 * VLA, atomics, threads are advertised; distinct same-named TLS variables in nested scopes retain separate values.
 * Expected: exit 0; stdout: ok; mode: c11.
 */
#include <stdio.h>
#define CHECK(x) do { if (!(x)) return 1; } while (0)
#include <threads.h>
#if defined(__STDC_NO_VLA__) || defined(__STDC_NO_ATOMICS__) || defined(__STDC_NO_THREADS__)
#error Feature unexpectedly unavailable
#endif
static int f(void) { static thread_local int x=1; int a=++x; { static thread_local int x=2; return 10*a + ++x; } }
int main(void) {
CHECK(f()==23); CHECK(f()==34);
    puts("ok"); return 0;
}
