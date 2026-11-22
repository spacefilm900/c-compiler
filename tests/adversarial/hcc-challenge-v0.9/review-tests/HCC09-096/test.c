#include <stdio.h>
#include <stdlib.h>
#define CK(x) do { if (!(x)) exit(1); } while (0)
#define OK() do { puts("ok"); return 0; } while (0)
#include <stdatomic.h>
int main(void){_Atomic(double) x;atomic_init(&x,1.5);double old=atomic_exchange(&x,2.5);CK(old==(1.5));double e=(1.5);CK(!atomic_compare_exchange_strong(&x,&e,(double)0));CK(e==(2.5));CK(atomic_compare_exchange_strong(&x,&e,(double)0));CK(atomic_load(&x)==0);OK();}
