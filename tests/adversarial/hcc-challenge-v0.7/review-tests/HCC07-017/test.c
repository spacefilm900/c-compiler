/* HCC07-017: K&R caller and prototype definition separate units
 * Basis: 6.7.6.3p14,p15; 6.5.2.2
 * Nonprototype call promotes float and narrow integer; separate definition uses promoted types.
 * Expected: exit 0; stdout: ok; mode: c11.
 */
#include <stdio.h>
#define CHECK(x) do { if (!(x)) return 1; } while (0)

extern int f();
int main(void) {
signed char c=7; float x=1.25f; CHECK(f(c,x)==12);
    puts("ok"); return 0;
}
