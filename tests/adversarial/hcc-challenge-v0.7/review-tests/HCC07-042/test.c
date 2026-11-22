/* HCC07-042: Nonconstant memory order evaluated once
 * Basis: 7.17.7.2; 6.5.2.2
 * Runtime order argument has side effect exactly once; stronger lowering is documented.
 * Expected: exit 0; stdout: ok; mode: c11.
 */
#include <stdio.h>
#define CHECK(x) do { if (!(x)) return 1; } while (0)
#include <stdatomic.h>

int main(void) {
atomic_int a=17; int n=0; memory_order orders[2]={memory_order_acquire,memory_order_relaxed}; int x=atomic_load_explicit(&a,orders[n++]); CHECK(n==1 && x==17);
    puts("ok"); return 0;
}
