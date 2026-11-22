#include <stdio.h>
#include <stdlib.h>
#define CK(x) do { if (!(x)) exit(1); } while (0)
#define OK() do { puts("ok"); return 0; } while (0)
#include <threads.h>
#include <stdatomic.h>
#include <time.h>
#define JOIN(t) CK(thrd_join(t,0)==thrd_success)
#define START(t,f,a) CK(thrd_create(&(t),f,a)==thrd_success)
enum{CAP=1,N=500};mtx_t m;cnd_t full,empty;int q[CAP],head,tail,count;int producer(void*p){(void)p;for(int i=1;i<=N;i++){CK(mtx_lock(&m)==thrd_success);while(count==CAP)CK(cnd_wait(&empty,&m)==thrd_success);q[tail]=i;tail=(tail+1)%CAP;count++;CK(cnd_signal(&full)==thrd_success);CK(mtx_unlock(&m)==thrd_success);}return 0;}int consumer(void*p){(void)p;for(int i=1;i<=N;i++){CK(mtx_lock(&m)==thrd_success);while(!count)CK(cnd_wait(&full,&m)==thrd_success);CK(q[head]==i);head=(head+1)%CAP;count--;CK(cnd_signal(&empty)==thrd_success);CK(mtx_unlock(&m)==thrd_success);}return 0;}int main(void){thrd_t t,u;CK(mtx_init(&m,mtx_plain)==thrd_success);CK(cnd_init(&full)==thrd_success);CK(cnd_init(&empty)==thrd_success);START(t,producer,0);START(u,consumer,0);JOIN(t);JOIN(u);CK(count==0);cnd_destroy(&full);cnd_destroy(&empty);mtx_destroy(&m);OK();}
