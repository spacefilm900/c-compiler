/* HCC08-029: CAS success and failure with distinct expected values
 * 6.5.16.2; 7.17.3; 7.17.7; 7.17.8. Defined operation and exact state checks; no padding comparison. */
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#define CHECK(x) do { if (!(x)) return __LINE__; } while(0)
#include <stdatomic.h>

int main(void){
atomic_int x=ATOMIC_VAR_INIT(12); int e=11; CHECK(!atomic_compare_exchange_strong_explicit(&x,&e,20,memory_order_acq_rel,memory_order_acquire)); CHECK(e==12); CHECK(atomic_compare_exchange_strong(&x,&e,20)); CHECK(e==12 && atomic_load(&x)==20);
puts("ok");return 0;
}
