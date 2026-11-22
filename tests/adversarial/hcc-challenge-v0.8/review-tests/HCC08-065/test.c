/* HCC08-065: GNU overflow builtin narrow output
 * GCC C extensions; N1570 6.5, 6.7, 6.8 as applicable. Documented GNU feature combined with ordinary C semantics. */
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#define CHECK(x) do { if (!(x)) return __LINE__; } while(0)

int main(void){
unsigned char out=0;unsigned int a=255,b=2;CHECK(__builtin_add_overflow(a,b,&out)&&out==1);CHECK(!__builtin_sub_overflow(a,b,&out)&&out==253);
puts("ok");return 0;
}
