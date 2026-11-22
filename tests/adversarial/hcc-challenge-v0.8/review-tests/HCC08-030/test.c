/* HCC08-030: Weak CAS retry after mismatch
 * 6.5.16.2; 7.17.3; 7.17.7; 7.17.8. Defined operation and exact state checks; no padding comparison. */
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#define CHECK(x) do { if (!(x)) return __LINE__; } while(0)
#include <stdatomic.h>

int main(void){
atomic_int x=ATOMIC_VAR_INIT(3); int e=2; CHECK(!atomic_compare_exchange_weak(&x,&e,8)); CHECK(e==3); int n=0; while(!atomic_compare_exchange_weak(&x,&e,8)){CHECK(e==3 && ++n<10000);} CHECK(atomic_load(&x)==8);
puts("ok");return 0;
}
