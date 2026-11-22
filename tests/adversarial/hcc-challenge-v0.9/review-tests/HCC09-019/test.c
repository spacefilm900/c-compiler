#include <stdio.h>
#include <stdlib.h>
#define CK(x) do { if (!(x)) exit(1); } while (0)
#define OK() do { puts("ok"); return 0; } while (0)
#include <threads.h>
#include <stdatomic.h>
#include <time.h>
#define JOIN(t) CK(thrd_join(t,0)==thrd_success)
#define START(t,f,a) CK(thrd_create(&(t),f,a)==thrd_success)
struct S{int a,b;};static _Thread_local int * x=0;int f(void*p){(void)p;CK(x==0);static int v;x=&v;return 0;}int main(void){thrd_t t;START(t,f,0);JOIN(t);CK(x==0);START(t,f,0);JOIN(t);CK(x==0);OK();}
