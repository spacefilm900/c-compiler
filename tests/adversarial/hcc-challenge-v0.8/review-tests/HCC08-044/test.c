/* HCC08-044: Condition variable broadcast predicate
 * 5.1.2.4; 6.2.4; 7.17; 7.26. Deterministic invariant after synchronisation; no timing-dependent expected value. */
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#define CHECK(x) do { if (!(x)) return __LINE__; } while(0)
#include <stdatomic.h>
#include <threads.h>
mtx_t m;cnd_t cv;int ready,go;int worker(void*p){(void)p;if(mtx_lock(&m)!=thrd_success)return 1;ready++;cnd_broadcast(&cv);while(!go)if(cnd_wait(&cv,&m)!=thrd_success)return 2;mtx_unlock(&m);return 0;}
int main(void){
CHECK(mtx_init(&m,mtx_plain)==thrd_success);CHECK(cnd_init(&cv)==thrd_success);thrd_t t[3];for(int i=0;i<3;i++)CHECK(thrd_create(&t[i],worker,0)==thrd_success);CHECK(mtx_lock(&m)==thrd_success);while(ready!=3)CHECK(cnd_wait(&cv,&m)==thrd_success);go=1;CHECK(cnd_broadcast(&cv)==thrd_success);CHECK(mtx_unlock(&m)==thrd_success);for(int i=0;i<3;i++){int v;CHECK(thrd_join(t[i],&v)==thrd_success&&v==0);}cnd_destroy(&cv);mtx_destroy(&m);
puts("ok");return 0;
}
