/* HCC08-061: GNU typeof on VLA pointer captures stride
 * GCC C extensions; N1570 6.5, 6.7, 6.8 as applicable. Documented GNU feature combined with ordinary C semantics. */
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#define CHECK(x) do { if (!(x)) return __LINE__; } while(0)

int main(void){
int n=4;int a[3][n];typedef typeof(a[0]) Row;n=6;Row *p=(Row*)a;p[2][3]=47;CHECK(a[2][3]==47&&sizeof(Row)==4*sizeof(int));
puts("ok");return 0;
}
