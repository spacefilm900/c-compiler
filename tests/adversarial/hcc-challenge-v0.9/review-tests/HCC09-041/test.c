#include <stdio.h>
#include <stdlib.h>
#define CK(x) do { if (!(x)) exit(1); } while (0)
#define OK() do { puts("ok"); return 0; } while (0)
#include <threads.h>
#include <stdatomic.h>
#include <time.h>
#define JOIN(t) CK(thrd_join(t,0)==thrd_success)
#define START(t,f,a) CK(thrd_create(&(t),f,a)==thrd_success)
atomic_int flag;int data;int a(void*p){(void)p;data=73;atomic_store_explicit(&flag,1,memory_order_release);return 0;}int b(void*p){(void)p;while(!atomic_load_explicit(&flag,memory_order_relaxed))thrd_yield();atomic_fetch_add_explicit(&flag,1,memory_order_relaxed);return 0;}int main(void){thrd_t t,u;START(t,a,0);START(u,b,0);while(atomic_load_explicit(&flag,memory_order_acquire)!=2)thrd_yield();CK(data==73);JOIN(t);JOIN(u);OK();}
