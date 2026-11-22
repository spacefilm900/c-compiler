#include <stdio.h>
#include <stdlib.h>
#define CK(x) do { if (!(x)) exit(1); } while (0)
#define OK() do { puts("ok"); return 0; } while (0)
#include <threads.h>
#include <stdatomic.h>
#include <time.h>
#define JOIN(t) CK(thrd_join(t,0)==thrd_success)
#define START(t,f,a) CK(thrd_create(&(t),f,a)==thrd_success)
mtx_t m;int f(void*p){(void)p;struct timespec t;CK(timespec_get(&t,TIME_UTC)==TIME_UTC);return mtx_timedlock(&m,&t)==thrd_timedout?0:1;}int main(void){thrd_t t;int v;CK(mtx_init(&m,mtx_timed)==thrd_success);CK(mtx_lock(&m)==thrd_success);START(t,f,0);CK(thrd_join(t,&v)==thrd_success && v==0);CK(mtx_unlock(&m)==thrd_success);mtx_destroy(&m);OK();}
