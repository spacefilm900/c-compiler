#include <stdio.h>
#include <stdlib.h>
#define CK(x) do { if (!(x)) exit(1); } while (0)
#define OK() do { puts("ok"); return 0; } while (0)
#include <threads.h>
#include <stdatomic.h>
#include <time.h>
#define JOIN(t) CK(thrd_join(t,0)==thrd_success)
#define START(t,f,a) CK(thrd_create(&(t),f,a)==thrd_success)
int next(void){static _Thread_local int x=10;return ++x;}int f(void*p){(void)p;CK(next()==11);CK(next()==12);return 0;}int main(void){thrd_t t;CK(next()==11);START(t,f,0);JOIN(t);CK(next()==12);OK();}
