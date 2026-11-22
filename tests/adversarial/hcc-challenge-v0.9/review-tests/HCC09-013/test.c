#include <stdio.h>
#include <stdlib.h>
#define CK(x) do { if (!(x)) exit(1); } while (0)
#define OK() do { puts("ok"); return 0; } while (0)
#include <threads.h>
#include <stdatomic.h>
#include <time.h>
#define JOIN(t) CK(thrd_join(t,0)==thrd_success)
#define START(t,f,a) CK(thrd_create(&(t),f,a)==thrd_success)
mtx_t m;cnd_t c;int value;int f(void*p){(void)p;CK(mtx_lock(&m)==thrd_success);while(!value)CK(cnd_wait(&c,&m)==thrd_success);CK(value==7);CK(mtx_unlock(&m)==thrd_success);return 0;}int main(void){thrd_t t;CK(mtx_init(&m,mtx_plain)==thrd_success);CK(cnd_init(&c)==thrd_success);START(t,f,0);CK(mtx_lock(&m)==thrd_success);value=7;CK(cnd_signal(&c)==thrd_success);CK(mtx_unlock(&m)==thrd_success);JOIN(t);cnd_destroy(&c);mtx_destroy(&m);OK();}
