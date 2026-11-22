/* HCC07-038: Failed compare exchange updates expected
 * Basis: 7.17.7.4
 * Strong CAS failure changes expected to stored value, and successful retry installs desired.
 * Expected: exit 0; stdout: ok; mode: c11.
 */
#include <stdio.h>
#define CHECK(x) do { if (!(x)) return 1; } while (0)
#include <stdatomic.h>

int main(void) {
atomic_int a=6; int e=4; CHECK(!atomic_compare_exchange_strong_explicit(&a,&e,9,memory_order_acq_rel,memory_order_acquire)); CHECK(e==6 && atomic_load(&a)==6); CHECK(atomic_compare_exchange_strong(&a,&e,9)); CHECK(e==6 && atomic_load(&a)==9);
    puts("ok"); return 0;
}
