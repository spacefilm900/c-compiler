/* HCC08-043: Recursive mutex depth and trylock
 * 5.1.2.4; 6.2.4; 7.17; 7.26. Deterministic invariant after synchronisation; no timing-dependent expected value. */
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#define CHECK(x) do { if (!(x)) return __LINE__; } while(0)
#include <stdatomic.h>
#include <threads.h>
mtx_t m; int worker(void*p){(void)p;int r=mtx_trylock(&m);if(r==thrd_success)mtx_unlock(&m);return r==thrd_busy?0:1;}
int main(void){
CHECK(mtx_init(&m,mtx_recursive)==thrd_success);CHECK(mtx_lock(&m)==thrd_success);CHECK(mtx_lock(&m)==thrd_success);CHECK(mtx_unlock(&m)==thrd_success);thrd_t t;int r;CHECK(thrd_create(&t,worker,0)==thrd_success);CHECK(thrd_join(t,&r)==thrd_success&&r==0);CHECK(mtx_unlock(&m)==thrd_success);CHECK(mtx_trylock(&m)==thrd_success);CHECK(mtx_unlock(&m)==thrd_success);mtx_destroy(&m);
puts("ok");return 0;
}
