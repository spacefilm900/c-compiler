/* HCC07-062: Volatile atomic operand and atomic flag state
 * Basis: 6.7.3; 7.17.7.5; 7.17.8
 * Generic fetch operation accepts volatile atomic objects; flag test-and-set returns previous state.
 * Expected: exit 0; stdout: ok; mode: c11.
 */
#include <stdio.h>
#define CHECK(x) do { if (!(x)) return 1; } while (0)
#include <stdatomic.h>

int main(void) {
volatile atomic_uint a=7; atomic_flag f=ATOMIC_FLAG_INIT; CHECK(atomic_fetch_xor(&a,3)==7 && atomic_load(&a)==4); CHECK(!atomic_flag_test_and_set(&f)); CHECK(atomic_flag_test_and_set(&f)); atomic_flag_clear_explicit(&f,memory_order_release); CHECK(!atomic_flag_test_and_set_explicit(&f,memory_order_acquire));
    puts("ok"); return 0;
}
