#include <stdio.h>
#include <stdlib.h>
#define CK(x) do { if (!(x)) exit(1); } while (0)
#define OK() do { puts("ok"); return 0; } while (0)
#include <threads.h>
#include <stdatomic.h>
#include <time.h>
#define JOIN(t) CK(thrd_join(t,0)==thrd_success)
#define START(t,f,a) CK(thrd_create(&(t),f,a)==thrd_success)
atomic_int x,y;int a,b;int f(void*p){(void)p;x=1;a=y;return 0;}int g(void*p){(void)p;y=1;b=x;return 0;}int main(void){for(int i=0;i<50;i++){thrd_t t,u;x=0;y=0;START(t,f,0);START(u,g,0);JOIN(t);JOIN(u);CK(a||b);}OK();}
