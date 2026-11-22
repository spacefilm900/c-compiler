/* HCC08-047: TSS independent keys and deletion
 * 5.1.2.4; 6.2.4; 7.17; 7.26. Deterministic invariant after synchronisation; no timing-dependent expected value. */
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#define CHECK(x) do { if (!(x)) return __LINE__; } while(0)
#include <stdatomic.h>
#include <threads.h>
tss_t a,b;int worker(void*p){int x=1,y=2;if(tss_get(a)||tss_get(b))return 1;if(tss_set(a,&x)!=thrd_success||tss_set(b,&y)!=thrd_success)return 2;return tss_get(a)==&x&&tss_get(b)==&y?0:3;}
int main(void){
int n=5;CHECK(tss_create(&a,0)==thrd_success);CHECK(tss_create(&b,0)==thrd_success);CHECK(tss_set(a,&n)==thrd_success);thrd_t t;int v;CHECK(thrd_create(&t,worker,0)==thrd_success);CHECK(thrd_join(t,&v)==thrd_success&&v==0);CHECK(tss_get(a)==&n&&tss_get(b)==0);tss_delete(a);tss_delete(b);
puts("ok");return 0;
}
