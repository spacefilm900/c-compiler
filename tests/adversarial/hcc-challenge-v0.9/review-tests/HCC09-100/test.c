#include <stdio.h>
#include <stdlib.h>
#define CK(x) do { if (!(x)) exit(1); } while (0)
#define OK() do { puts("ok"); return 0; } while (0)
#include <stdatomic.h>
int main(void){volatile int x=1;atomic_signal_fence(memory_order_seq_cst);x+=2;atomic_signal_fence(memory_order_acq_rel);CK(x==3);OK();}
