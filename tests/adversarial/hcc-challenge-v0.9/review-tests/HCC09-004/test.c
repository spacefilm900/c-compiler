#include <stdio.h>
#include <stdlib.h>
#define CK(x) do { if (!(x)) exit(1); } while (0)
#define OK() do { puts("ok"); return 0; } while (0)
#include <threads.h>
#include <stdatomic.h>
#include <time.h>
#define JOIN(t) CK(thrd_join(t,0)==thrd_success)
#define START(t,f,a) CK(thrd_create(&(t),f,a)==thrd_success)
int child(void *p){return *(int*)p;} int f(void *p){thrd_t t;int v;START(t,child,p);CK(thrd_join(t,&v)==thrd_success);return v+1;} int main(void){int n=9,v;thrd_t t;START(t,f,&n);CK(thrd_join(t,&v)==thrd_success && v==10);OK();}
