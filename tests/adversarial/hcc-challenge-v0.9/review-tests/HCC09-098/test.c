#include <stdio.h>
#include <stdlib.h>
#define CK(x) do { if (!(x)) exit(1); } while (0)
#define OK() do { puts("ok"); return 0; } while (0)
#include <stdatomic.h>
int main(void){atomic_flag f=ATOMIC_FLAG_INIT;CK(!atomic_flag_test_and_set(&f));CK(atomic_flag_test_and_set(&f));atomic_flag_clear(&f);CK(!atomic_flag_test_and_set(&f));OK();}
