#include <stdio.h>
#include <stdlib.h>
#define CK(x) do { if (!(x)) exit(1); } while (0)
#define OK() do { puts("ok"); return 0; } while (0)
#include <threads.h>
#include <stdatomic.h>
#include <time.h>
#define JOIN(t) CK(thrd_join(t,0)==thrd_success)
#define START(t,f,a) CK(thrd_create(&(t),f,a)==thrd_success)
atomic_int done; int f(void *p){(void)p;atomic_store_explicit(&done,1,memory_order_release);return 0;} int main(void){thrd_t t;START(t,f,0);CK(thrd_detach(t)==thrd_success);while(!atomic_load_explicit(&done,memory_order_acquire)) thrd_yield();OK();}
