/* HCC08-068: GNU case range mixed with enum constant
 * GCC C extensions; N1570 6.5, 6.7, 6.8 as applicable. Documented GNU feature combined with ordinary C semantics. */
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#define CHECK(x) do { if (!(x)) return __LINE__; } while(0)

int main(void){
enum{LOW=3,HIGH=7};int z=0;for(int i=0;i<10;i++){switch(i){case LOW ... HIGH:z+=i;break;default:break;}}CHECK(z==25);
puts("ok");return 0;
}
