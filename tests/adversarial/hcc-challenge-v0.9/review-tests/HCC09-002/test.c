#include <stdio.h>
#include <stdlib.h>
#define CK(x) do { if (!(x)) exit(1); } while (0)
#define OK() do { puts("ok"); return 0; } while (0)
#include <threads.h>
#include <stdatomic.h>
#include <time.h>
#define JOIN(t) CK(thrd_join(t,0)==thrd_success)
#define START(t,f,a) CK(thrd_create(&(t),f,a)==thrd_success)
int f(void *p){(void)p;thrd_exit(37);return 0;} int main(void){int v;thrd_t t;START(t,f,0);CK(thrd_join(t,&v)==thrd_success && v==37);OK();}
