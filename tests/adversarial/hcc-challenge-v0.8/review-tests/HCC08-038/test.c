/* HCC08-038: Atomic 16-byte aggregate CAS
 * 6.5.16.2; 7.17.3; 7.17.7; 7.17.8. Defined operation and exact state checks; no padding comparison. */
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#define CHECK(x) do { if (!(x)) return __LINE__; } while(0)
#include <stdatomic.h>

int main(void){
struct Pair {uint64_t a,b;}; _Atomic(struct Pair) x; struct Pair p={2,7},q={9,11},e={2,8}; atomic_init(&x,p); CHECK(!atomic_compare_exchange_strong(&x,&e,q)); CHECK(e.a==2 && e.b==7); CHECK(atomic_compare_exchange_strong(&x,&e,q)); struct Pair now=atomic_load(&x); CHECK(now.a==9 && now.b==11);
puts("ok");return 0;
}
