/* HCC07-039: Atomic floating exchange and compound RMW
 * Basis: 6.5.16.2p3; 7.17.7.3
 * Atomic floating exchange and multiplication operate on represented floating values, not integer bit patterns.
 * Expected: exit 0; stdout: ok; mode: c11.
 */
#include <stdio.h>
#define CHECK(x) do { if (!(x)) return 1; } while (0)
#include <stdatomic.h>

int main(void) {
_Atomic double a=1.5; double old=atomic_exchange(&a,2.0); CHECK(old==1.5); a*=2.5; a-=1.0; double now=a; CHECK(now==4.0);
    puts("ok"); return 0;
}
