#include <stdio.h>
#include <stdlib.h>
#define CK(x) do { if (!(x)) exit(1); } while (0)
#define OK() do { puts("ok"); return 0; } while (0)
#include <threads.h>
#include <stdatomic.h>
#include <time.h>
#define JOIN(t) CK(thrd_join(t,0)==thrd_success)
#define START(t,f,a) CK(thrd_create(&(t),f,a)==thrd_success)
tss_t k;int f(void*p){CK(tss_get(k)==0);CK(tss_set(k,p)==thrd_success);CK(tss_get(k)==p);return 0;}int main(void){int a=1,b=2;thrd_t t;CK(tss_create(&k,0)==thrd_success);CK(tss_set(k,&a)==thrd_success);START(t,f,&b);JOIN(t);CK(tss_get(k)==&a);tss_delete(k);OK();}
