#include <stdio.h>
#include <stdlib.h>
#define CK(x) do { if (!(x)) exit(1); } while (0)
#define OK() do { puts("ok"); return 0; } while (0)
#include <threads.h>
#include <stdatomic.h>
#include <time.h>
#define JOIN(t) CK(thrd_join(t,0)==thrd_success)
#define START(t,f,a) CK(thrd_create(&(t),f,a)==thrd_success)
atomic_uint n=4000;int f(void*p){for(int i=0;i<1000;i++){atomic_fetch_sub_explicit(&n,1u,memory_order_relaxed);}return 0;}int main(void){thrd_t t[4];int id[4]={0,1,2,3};for(int i=0;i<4;i++)START(t[i],f,&id[i]);for(int i=0;i<4;i++)JOIN(t[i]);CK(n==0);OK();}
