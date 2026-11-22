#include <stdio.h>
#include <stdlib.h>
#define CK(x) do { if (!(x)) exit(1); } while (0)
#define OK() do { puts("ok"); return 0; } while (0)
#include <threads.h>
#include <stdatomic.h>
#include <time.h>
#define JOIN(t) CK(thrd_join(t,0)==thrd_success)
#define START(t,f,a) CK(thrd_create(&(t),f,a)==thrd_success)
tss_t k;atomic_int count;static int val;void d(void*p){CK(p==&val);int n=atomic_fetch_add(&count,1);if(n==0)CK(tss_set(k,&val)==thrd_success);}int f(void*p){(void)p;CK(tss_set(k,&val)==thrd_success);thrd_exit(0);return 0;}int main(void){thrd_t t;CK(TSS_DTOR_ITERATIONS>=2);CK(tss_create(&k,d)==thrd_success);START(t,f,0);JOIN(t);CK(atomic_load(&count)==2);tss_delete(k);OK();}
