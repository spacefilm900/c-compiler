#include <stdio.h>
#include <stdlib.h>
#define CK(x) do { if (!(x)) exit(1); } while (0)
#define OK() do { puts("ok"); return 0; } while (0)
#include <threads.h>
#include <stdatomic.h>
#include <time.h>
#define JOIN(t) CK(thrd_join(t,0)==thrd_success)
#define START(t,f,a) CK(thrd_create(&(t),f,a)==thrd_success)
tss_t k;atomic_int count;static int val;void d(void*p){(void)p;atomic_fetch_add(&count,1);}int f(void*p){(void)p;CK(tss_set(k,&val)==thrd_success);CK(tss_set(k,0)==thrd_success);return 0;}int main(void){thrd_t t;CK(tss_create(&k,d)==thrd_success);START(t,f,0);JOIN(t);CK(atomic_load(&count)==0);tss_delete(k);OK();}
