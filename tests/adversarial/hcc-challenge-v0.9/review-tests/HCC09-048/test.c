#include <stdio.h>
#include <stdlib.h>
#define CK(x) do { if (!(x)) exit(1); } while (0)
#define OK() do { puts("ok"); return 0; } while (0)
#include <threads.h>
#include <stdatomic.h>
#include <time.h>
#define JOIN(t) CK(thrd_join(t,0)==thrd_success)
#define START(t,f,a) CK(thrd_create(&(t),f,a)==thrd_success)
enum{N=32,K=16};atomic_uint bins[K];int f(void*p){int id=*(int*)p;for(int i=0;i<1024;i++)atomic_fetch_add_explicit(&bins[(i+id)%K],1,memory_order_relaxed);return 0;}int main(void){thrd_t t[N];int id[N];for(int i=0;i<N;i++){id[i]=i;START(t[i],f,&id[i]);}for(int i=0;i<N;i++)JOIN(t[i]);for(int i=0;i<K;i++)CK(bins[i]==N*64);OK();}
