#include <stdio.h>
#include <stdlib.h>
#define CK(x) do { if (!(x)) exit(1); } while (0)
#define OK() do { puts("ok"); return 0; } while (0)
#include <threads.h>
#include <stdatomic.h>
#include <time.h>
#define JOIN(t) CK(thrd_join(t,0)==thrd_success)
#define START(t,f,a) CK(thrd_create(&(t),f,a)==thrd_success)
int main(void){mtx_t m;CK(mtx_init(&m,mtx_plain|mtx_recursive)==thrd_success);CK(mtx_lock(&m)==thrd_success);CK(mtx_trylock(&m)==thrd_success);CK(mtx_lock(&m)==thrd_success);for(int i=0;i<3;i++)CK(mtx_unlock(&m)==thrd_success);mtx_destroy(&m);OK();}
