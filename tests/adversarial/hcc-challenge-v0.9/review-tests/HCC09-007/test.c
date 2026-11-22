#include <stdio.h>
#include <stdlib.h>
#define CK(x) do { if (!(x)) exit(1); } while (0)
#define OK() do { puts("ok"); return 0; } while (0)
#include <threads.h>
#include <stdatomic.h>
#include <time.h>
#define JOIN(t) CK(thrd_join(t,0)==thrd_success)
#define START(t,f,a) CK(thrd_create(&(t),f,a)==thrd_success)
mtx_t m;int n;int f(void*p){(void)p;for(int i=0;i<300;i++){CK(mtx_lock(&m)==thrd_success);n++;CK(mtx_unlock(&m)==thrd_success);}return 0;}int main(void){thrd_t t[4];CK(mtx_init(&m,mtx_plain)==thrd_success);for(int i=0;i<4;i++)START(t[i],f,0);for(int i=0;i<4;i++)JOIN(t[i]);CK(n==1200);mtx_destroy(&m);OK();}
