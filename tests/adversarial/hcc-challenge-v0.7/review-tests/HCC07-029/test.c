/* HCC07-029: Static pointer to VLA type in block
 * Basis: 6.7.6.2p2; 6.2.4
 * Block-scope static objects of pointer-to-VLA type are permitted when the identifier has no linkage.
 * Expected: exit 0; stdout: ok; mode: c11.
 */
#include <stdio.h>
#define CHECK(x) do { if (!(x)) return 1; } while (0)

int f(int n) { static int (*p)[n]; int a[n]; p=&a; int r=(int)(sizeof *p / sizeof(int)); p=0; return r; }
int main(void) {
CHECK(f(3)==3); CHECK(f(5)==5);
    puts("ok"); return 0;
}
