/* HCC07-055: Computed goto with VLA declared outside targets
 * Basis: N1570 6.2.4; GCC manual Labels as Values
 * Indirect jump between labels wholly within already-live VLA scope does not enter a new VM scope.
 * Expected: exit 0; stdout: ok; mode: gnu11.
 */
#include <stdio.h>
#define CHECK(x) do { if (!(x)) return 1; } while (0)

static int f(int k,int n){ int a[n]; a[n-1]=7; void *p=k?&&B:&&A; goto *p; A:return a[n-1]; B:return a[n-1]+4; }
int main(void) {
CHECK(f(0,3)==7 && f(1,5)==11);
    puts("ok"); return 0;
}
