#include <stdio.h>
#include <stdlib.h>
#define CK(x) do { if (!(x)) exit(1); } while (0)
#define OK() do { puts("ok"); return 0; } while (0)
#include <threads.h>
#include <stdatomic.h>
#include <time.h>
#define JOIN(t) CK(thrd_join(t,0)==thrd_success)
#define START(t,f,a) CK(thrd_create(&(t),f,a)==thrd_success)
_Atomic(unsigned char) n;int f(void*p){(void)p;for(int i=0;i<40;i++){unsigned char old=atomic_load_explicit(&n,memory_order_relaxed);while(!atomic_compare_exchange_weak_explicit(&n,&old,(unsigned char)(old+1),memory_order_relaxed,memory_order_relaxed)){}}return 0;}int main(void){thrd_t t[4];for(int i=0;i<4;i++)START(t[i],f,0);for(int i=0;i<4;i++)JOIN(t[i]);CK(n==160);OK();}
