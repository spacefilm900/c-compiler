/* HCC07-051: typeof VLA captures operand type from runtime bound
 * Basis: N1570 6.7.6.2; GCC manual Typeof
 * GNU typeof of variably modified type preserves runtime stride; plain typeof is opt-in.
 * Expected: exit 0; stdout: ok; mode: gnu11.
 */
#include <stdio.h>
#define CHECK(x) do { if (!(x)) return 1; } while (0)


int main(void) {
int n=4; int a[2][n]; typeof(a[0]) b; b[3]=19; CHECK(sizeof b==4*sizeof(int) && b[3]==19);
    puts("ok"); return 0;
}
