#include <stdio.h>
#include <stdlib.h>
#define CK(x) do { if (!(x)) exit(1); } while (0)
#define OK() do { puts("ok"); return 0; } while (0)
#include <stdatomic.h>
int main(void){_Atomic(unsigned long) x;atomic_init(&x,19);unsigned long old=atomic_exchange(&x,42);CK(old==(19));unsigned long e=(19);CK(!atomic_compare_exchange_strong(&x,&e,(unsigned long)0));CK(e==(42));CK(atomic_compare_exchange_strong(&x,&e,(unsigned long)0));CK(atomic_load(&x)==0);OK();}
