/* HCC08-045: TLS nested over-aligned object in workers
 * 5.1.2.4; 6.2.4; 7.17; 7.26. Deterministic invariant after synchronisation; no timing-dependent expected value. */
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#define CHECK(x) do { if (!(x)) return __LINE__; } while(0)
#include <stdatomic.h>
#include <threads.h>
struct I{char x;_Alignas(64) int v;};struct O{char x;struct I a[2];};static _Thread_local struct O o;int worker(void*p){int n=*(int*)p;if(o.a[1].v!=0||(uintptr_t)&o.a[1].v%64)return 1;o.a[1].v=n;thrd_yield();return o.a[1].v==n?0:2;}
int main(void){
thrd_t t[3]; int n[3]={11,23,37};o.a[1].v=91;for(int i=0;i<3;i++)CHECK(thrd_create(&t[i],worker,&n[i])==thrd_success);for(int i=0;i<3;i++){int v;CHECK(thrd_join(t[i],&v)==thrd_success&&v==0);}CHECK(o.a[1].v==91);
puts("ok");return 0;
}
