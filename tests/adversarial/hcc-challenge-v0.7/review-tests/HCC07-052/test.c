/* HCC07-052: Statement expression and VLA scope
 * Basis: N1570 6.2.4; GCC manual Statement Exprs
 * Statement expression yields final expression value before VLA lifetime ends.
 * Expected: exit 0; stdout: ok; mode: gnu11.
 */
#include <stdio.h>
#define CHECK(x) do { if (!(x)) return 1; } while (0)


int main(void) {
int n=5; int sum=0; for(int i=0;i<10000;i++) sum+=({ int a[n]; a[4]=2; a[4]+1; }); CHECK(sum==30000);
    puts("ok"); return 0;
}
