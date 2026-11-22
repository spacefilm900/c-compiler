#include <stdio.h>
#include <stdlib.h>
#define CK(x) do { if (!(x)) exit(1); } while (0)
#define OK() do { puts("ok"); return 0; } while (0)
#include <threads.h>
#include <stdatomic.h>
#include <time.h>
#define JOIN(t) CK(thrd_join(t,0)==thrd_success)
#define START(t,f,a) CK(thrd_create(&(t),f,a)==thrd_success)
thrd_t parent; int f(void *p){(void)p;return thrd_equal(thrd_current(),parent)?1:0;} int main(void){parent=thrd_current();thrd_t t;int v;START(t,f,0);CK(thrd_join(t,&v)==thrd_success && v==0);CK(thrd_equal(parent,thrd_current()));OK();}
