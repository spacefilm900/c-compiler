#include <stdarg.h>
struct S{unsigned a:3;unsigned :0;_Alignas(32) int x;unsigned b:5;}; struct S trans(struct S);int vari(int,...);
