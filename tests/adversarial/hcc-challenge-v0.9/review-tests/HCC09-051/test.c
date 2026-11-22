#include <stdio.h>
#include <stdlib.h>
#define CK(x) do { if (!(x)) exit(1); } while (0)
#define OK() do { puts("ok"); return 0; } while (0)
#include <threads.h>
#include <stdatomic.h>
#include <time.h>
#define JOIN(t) CK(thrd_join(t,0)==thrd_success)
#define START(t,f,a) CK(thrd_create(&(t),f,a)==thrd_success)
enum{CAP=8,N=1024};atomic_uint head,tail;int buf[CAP];int prod(void*p){(void)p;for(unsigned i=0;i<N;i++){while(i-atomic_load_explicit(&head,memory_order_acquire)==CAP)thrd_yield();buf[i%CAP]=(int)i;atomic_store_explicit(&tail,i+1,memory_order_release);}return 0;}int main(void){thrd_t t;START(t,prod,0);for(unsigned i=0;i<N;i++){while(atomic_load_explicit(&tail,memory_order_acquire)==i)thrd_yield();CK(buf[i%CAP]==(int)i);atomic_store_explicit(&head,i+1,memory_order_release);}JOIN(t);OK();}
