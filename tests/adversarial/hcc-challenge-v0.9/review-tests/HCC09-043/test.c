#include <stdio.h>
#include <stdlib.h>
#define CK(x) do { if (!(x)) exit(1); } while (0)
#define OK() do { puts("ok"); return 0; } while (0)
#include <threads.h>
#include <stdatomic.h>
#include <time.h>
#define JOIN(t) CK(thrd_join(t,0)==thrd_success)
#define START(t,f,a) CK(thrd_create(&(t),f,a)==thrd_success)
atomic_flag lock=ATOMIC_FLAG_INIT;int n;int f(void*p){(void)p;for(int i=0;i<300;i++){while(atomic_flag_test_and_set_explicit(&lock,memory_order_acquire))thrd_yield();n++;atomic_flag_clear_explicit(&lock,memory_order_release);}return 0;}int main(void){thrd_t t[4];for(int i=0;i<4;i++)START(t[i],f,0);for(int i=0;i<4;i++)JOIN(t[i]);CK(n==1200);OK();}
