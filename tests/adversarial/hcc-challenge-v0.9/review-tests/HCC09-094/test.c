#include <stdio.h>
#include <stdlib.h>
#define CK(x) do { if (!(x)) exit(1); } while (0)
#define OK() do { puts("ok"); return 0; } while (0)
#include <stdatomic.h>
int main(void){_Atomic(int) x;atomic_init(&x,-3);int old=atomic_exchange(&x,7);CK(old==(-3));int e=(-3);CK(!atomic_compare_exchange_strong(&x,&e,(int)0));CK(e==(7));CK(atomic_compare_exchange_strong(&x,&e,(int)0));CK(atomic_load(&x)==0);OK();}
