#include <stdio.h>
#include <stdlib.h>
#define CK(x) do { if (!(x)) exit(1); } while (0)
#define OK() do { puts("ok"); return 0; } while (0)
#include "shared.h"
int main(void){struct S s={.a=3,.x=21,.b=7};CK(vari(2,s,11)==44);OK();}
