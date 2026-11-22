#include <stdio.h>
#include <stdlib.h>
#define CK(x) do { if (!(x)) exit(1); } while (0)
#define OK() do { puts("ok"); return 0; } while (0)
#include <threads.h>
#include <stdatomic.h>
#include <time.h>
#define JOIN(t) CK(thrd_join(t,0)==thrd_success)
#define START(t,f,a) CK(thrd_create(&(t),f,a)==thrd_success)
int f(void *p){return *(int*)p+2;} int main(void){int x=19,v;thrd_t t;START(t,f,&x);CK(thrd_join(t,&v)==thrd_success && v==21);OK();}
