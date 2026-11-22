/* HCC08-062: GNU statement expression returns record by value
 * GCC C extensions; N1570 6.5, 6.7, 6.8 as applicable. Documented GNU feature combined with ordinary C semantics. */
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#define CHECK(x) do { if (!(x)) return __LINE__; } while(0)
struct R{int a;_Alignas(32) int b;};
int main(void){
struct R x=({struct R q={3,7};q;});CHECK(x.a==3&&x.b==7);
puts("ok");return 0;
}
