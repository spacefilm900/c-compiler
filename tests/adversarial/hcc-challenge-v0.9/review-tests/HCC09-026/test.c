#include <stdio.h>
#include <stdlib.h>
#define CK(x) do { if (!(x)) exit(1); } while (0)
#define OK() do { puts("ok"); return 0; } while (0)
#include <threads.h>
#include <stdatomic.h>
#include <time.h>
#define JOIN(t) CK(thrd_join(t,0)==thrd_success)
#define START(t,f,a) CK(thrd_create(&(t),f,a)==thrd_success)
once_flag flag=ONCE_FLAG_INIT;int count,data[4];void init(void){count++;for(int i=0;i<4;i++)data[i]=i+10;}int f(void*p){(void)p;call_once(&flag,init);CK(count==1 && data[3]==13);return 0;}int main(void){thrd_t t[8];for(int i=0;i<8;i++)START(t[i],f,0);for(int i=0;i<8;i++)JOIN(t[i]);CK(count==1);OK();}
