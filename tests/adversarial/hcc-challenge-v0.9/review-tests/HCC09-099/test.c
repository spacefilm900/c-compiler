#include <stdio.h>
#include <stdlib.h>
#define CK(x) do { if (!(x)) exit(1); } while (0)
#define OK() do { puts("ok"); return 0; } while (0)
#include <stdatomic.h>
int calls;memory_order order(void){calls++;return memory_order_relaxed;}int main(void){atomic_int x=5;CK(atomic_load_explicit(&x,order())==5);CK(calls==1);atomic_store_explicit(&x,7,order());CK(calls==2 && x==7);OK();}
