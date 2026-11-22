#include <stdio.h>
#include <stdlib.h>
#define CK(x) do { if (!(x)) exit(1); } while (0)
#define OK() do { puts("ok"); return 0; } while (0)
#include <threads.h>
#include <stdatomic.h>
#include <time.h>
#define JOIN(t) CK(thrd_join(t,0)==thrd_success)
#define START(t,f,a) CK(thrd_create(&(t),f,a)==thrd_success)
atomic_int next;int out[200];int f(void*p){(void)p;for(;;){int i=atomic_fetch_add(&next,1);if(i>=200)break;out[i]=i*i;}return 0;}int main(void){thrd_t t[8];for(int i=0;i<8;i++)START(t[i],f,0);for(int i=0;i<8;i++)JOIN(t[i]);for(int i=0;i<200;i++)CK(out[i]==i*i);OK();}
