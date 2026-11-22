/* HCC07-054: Case range with negative endpoints
 * Basis: N1570 6.8.4.2; GCC manual Case Ranges
 * Inclusive GNU range uses promoted switch values, including negative boundaries.
 * Expected: exit 0; stdout: ok; mode: gnu11.
 */
#include <stdio.h>
#define CHECK(x) do { if (!(x)) return 1; } while (0)

static int f(int x){switch(x){case -4 ... -2:return 1;case -1 ... 2:return 2;default:return 0;}}
int main(void) {
CHECK(f(-5)==0 && f(-4)==1 && f(-2)==1 && f(-1)==2 && f(2)==2 && f(3)==0);
    puts("ok"); return 0;
}
