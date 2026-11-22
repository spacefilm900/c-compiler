/* HCC08-041: CAS contention exact count
 * 5.1.2.4; 6.2.4; 7.17; 7.26. Deterministic invariant after synchronisation; no timing-dependent expected value. */
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#define CHECK(x) do { if (!(x)) return __LINE__; } while(0)
#include <stdatomic.h>
#include <threads.h>
atomic_int x=ATOMIC_VAR_INIT(0); int worker(void *p){(void)p;for(int i=0;i<3000;i++){int e=atomic_load(&x);while(!atomic_compare_exchange_weak(&x,&e,e+1)){} }return 0;}
int main(void){
thrd_t t[4];for(int i=0;i<4;i++)CHECK(thrd_create(&t[i],worker,0)==thrd_success);for(int i=0;i<4;i++){int r;CHECK(thrd_join(t[i],&r)==thrd_success && r==0);}CHECK(x==12000);
puts("ok");return 0;
}
