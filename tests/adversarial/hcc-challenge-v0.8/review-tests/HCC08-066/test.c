/* HCC08-066: GNU statement expression nested loop VLA exit
 * GCC C extensions; N1570 6.5, 6.7, 6.8 as applicable. Documented GNU feature combined with ordinary C semantics. */
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#define CHECK(x) do { if (!(x)) return __LINE__; } while(0)

int main(void){
int n=5,sum=0;for(int k=0;k<8;k++){int v=({int a[n+k];a[0]=k;a[0]+1;});sum+=v;}CHECK(sum==36);
puts("ok");return 0;
}
