/* HCC07-026: sizeof operand with VLA type evaluates pointer increment
 * Basis: 6.5.3.4p2; 6.5.2.4
 * For VLA operand, sizeof evaluates its operand; pointer advances exactly once.
 * Expected: exit 0; stdout: ok; mode: c11.
 */
#include <stdio.h>
#define CHECK(x) do { if (!(x)) return 1; } while (0)


int main(void) {
int n=3; int a[3][n]; int (*p)[n]=a; unsigned long z=sizeof *p++;
    CHECK(z==3*sizeof(int) && p==a+1);
    puts("ok"); return 0;
}
