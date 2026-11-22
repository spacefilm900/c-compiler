#include <stdio.h>
#include <stdlib.h>
#define CK(x) do { if (!(x)) exit(1); } while (0)
#define OK() do { puts("ok"); return 0; } while (0)
#include <threads.h>
#include <stdatomic.h>
#include <time.h>
#define JOIN(t) CK(thrd_join(t,0)==thrd_success)
#define START(t,f,a) CK(thrd_create(&(t),f,a)==thrd_success)
#include <stdint.h>
struct S{_Alignas(32) int a[8];};static _Thread_local struct S s;int f(void*p){int n=*(int*)p;CK(s.a[0]==0);CK((uintptr_t)&s%32==0);for(int i=0;i<8;i++)s.a[i]=n+i;thrd_yield();for(int i=0;i<8;i++)CK(s.a[i]==n+i);return 0;}int main(void){thrd_t t[4];int a[4]={1,2,3,4};for(int i=0;i<4;i++)START(t[i],f,&a[i]);for(int i=0;i<4;i++)JOIN(t[i]);CK(s.a[0]==0);OK();}
