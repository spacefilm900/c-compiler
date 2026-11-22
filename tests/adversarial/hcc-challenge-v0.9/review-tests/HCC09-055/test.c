#include <stdio.h>
#include <stdlib.h>
#define CK(x) do { if (!(x)) exit(1); } while (0)
#define OK() do { puts("ok"); return 0; } while (0)
#include <threads.h>
#include <stdatomic.h>
#include <time.h>
#define JOIN(t) CK(thrd_join(t,0)==thrd_success)
#define START(t,f,a) CK(thrd_create(&(t),f,a)==thrd_success)
atomic_int refs=4,done;int data[4];int f(void*p){int i=*(int*)p;data[i]=i+11;if(atomic_fetch_sub_explicit(&refs,1,memory_order_acq_rel)==1){for(int j=0;j<4;j++)CK(data[j]==j+11);atomic_store(&done,1);}return 0;}int main(void){thrd_t t[4];int a[4]={0,1,2,3};for(int i=0;i<4;i++)START(t[i],f,&a[i]);for(int i=0;i<4;i++)JOIN(t[i]);CK(done==1 && refs==0);OK();}
