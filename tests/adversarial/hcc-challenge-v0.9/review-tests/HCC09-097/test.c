#include <stdio.h>
#include <stdlib.h>
#define CK(x) do { if (!(x)) exit(1); } while (0)
#define OK() do { puts("ok"); return 0; } while (0)
#include <stdatomic.h>
int main(void){_Atomic(float) x;atomic_init(&x,1.25f);float old=atomic_exchange(&x,3.5f);CK(old==(1.25f));float e=(1.25f);CK(!atomic_compare_exchange_strong(&x,&e,(float)0));CK(e==(3.5f));CK(atomic_compare_exchange_strong(&x,&e,(float)0));CK(atomic_load(&x)==0);OK();}
