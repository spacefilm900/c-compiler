#include <stdio.h>
#include <stdlib.h>
#define CK(x) do { if (!(x)) exit(1); } while (0)
#define OK() do { puts("ok"); return 0; } while (0)
#include <threads.h>
#include <stdatomic.h>
#include <time.h>
#define JOIN(t) CK(thrd_join(t,0)==thrd_success)
#define START(t,f,a) CK(thrd_create(&(t),f,a)==thrd_success)
int main(void){mtx_t m;cnd_t c;struct timespec t;CK(mtx_init(&m,mtx_plain)==thrd_success);CK(cnd_init(&c)==thrd_success);CK(mtx_lock(&m)==thrd_success);CK(timespec_get(&t,TIME_UTC)==TIME_UTC);int rc;do{rc=cnd_timedwait(&c,&m,&t);}while(rc==thrd_success);CK(rc==thrd_timedout);CK(mtx_unlock(&m)==thrd_success);cnd_destroy(&c);mtx_destroy(&m);OK();}
