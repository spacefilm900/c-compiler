#include <stdio.h>
#include <stdlib.h>
#define CK(x) do { if (!(x)) exit(1); } while (0)
#define OK() do { puts("ok"); return 0; } while (0)
#include <threads.h>
#include <stdatomic.h>
#include <time.h>
#define JOIN(t) CK(thrd_join(t,0)==thrd_success)
#define START(t,f,a) CK(thrd_create(&(t),f,a)==thrd_success)
atomic_int flag;int data[32];int f(void*p){(void)p;for(int i=0;i<32;i++)data[i]=i*3;atomic_thread_fence(memory_order_release);atomic_store_explicit(&flag,1,memory_order_relaxed);return 0;}int main(void){thrd_t t;START(t,f,0);while(!atomic_load_explicit(&flag,memory_order_relaxed))thrd_yield();atomic_thread_fence(memory_order_acquire);for(int i=0;i<32;i++)CK(data[i]==i*3);JOIN(t);OK();}
