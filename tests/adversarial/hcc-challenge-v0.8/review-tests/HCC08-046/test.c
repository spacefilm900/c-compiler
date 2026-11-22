/* HCC08-046: Release sequence RMW publishes payload
 * 5.1.2.4; 6.2.4; 7.17; 7.26. Deterministic invariant after synchronisation; no timing-dependent expected value. */
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#define CHECK(x) do { if (!(x)) return __LINE__; } while(0)
#include <stdatomic.h>
#include <threads.h>
atomic_int stage=ATOMIC_VAR_INIT(0);int payload;int producer(void*p){(void)p;payload=73;atomic_store_explicit(&stage,1,memory_order_release);return 0;}int relay(void*p){(void)p;while(atomic_load_explicit(&stage,memory_order_relaxed)!=1)thrd_yield();atomic_fetch_add_explicit(&stage,1,memory_order_relaxed);return 0;}int consumer(void*p){(void)p;while(atomic_load_explicit(&stage,memory_order_acquire)!=2)thrd_yield();return payload==73?0:1;}
int main(void){
thrd_t t[3];CHECK(thrd_create(&t[0],producer,0)==thrd_success);CHECK(thrd_create(&t[1],relay,0)==thrd_success);CHECK(thrd_create(&t[2],consumer,0)==thrd_success);for(int i=0;i<3;i++){int v;CHECK(thrd_join(t[i],&v)==thrd_success&&v==0);}
puts("ok");return 0;
}
