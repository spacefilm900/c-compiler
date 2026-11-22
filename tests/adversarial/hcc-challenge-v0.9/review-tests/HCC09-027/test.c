#include <stdio.h>
#include <stdlib.h>
#define CK(x) do { if (!(x)) exit(1); } while (0)
#define OK() do { puts("ok"); return 0; } while (0)
#include <threads.h>
#include <stdatomic.h>
#include <time.h>
#define JOIN(t) CK(thrd_join(t,0)==thrd_success)
#define START(t,f,a) CK(thrd_create(&(t),f,a)==thrd_success)
once_flag a=ONCE_FLAG_INIT,b=ONCE_FLAG_INIT;int x,y;void f(void){x++;}void g(void){y+=2;}int worker(void*p){(void)p;call_once(&a,f);call_once(&b,g);call_once(&a,f);CK(x==1 && y==2);return 0;}int main(void){thrd_t t[4];for(int i=0;i<4;i++)START(t[i],worker,0);for(int i=0;i<4;i++)JOIN(t[i]);OK();}
