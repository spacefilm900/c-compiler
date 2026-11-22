#include <stdio.h>
#include <stdlib.h>
#define CK(x) do { if (!(x)) exit(1); } while (0)
#define OK() do { puts("ok"); return 0; } while (0)
#include <threads.h>
#include <stdatomic.h>
#include <time.h>
#define JOIN(t) CK(thrd_join(t,0)==thrd_success)
#define START(t,f,a) CK(thrd_create(&(t),f,a)==thrd_success)
int main(void){mtx_t m;struct timespec t;CK(timespec_get(&t,TIME_UTC)==TIME_UTC);t.tv_sec+=2;CK(mtx_init(&m,mtx_timed)==thrd_success);CK(mtx_timedlock(&m,&t)==thrd_success);CK(mtx_unlock(&m)==thrd_success);mtx_destroy(&m);OK();}
