/* HCC07-025: VLA pointer stride across three dimensions
 * Basis: 6.7.6.2; 6.5.6p8,p9
 * Runtime strides and pointer subtraction use complete element size.
 * Expected: exit 0; stdout: ok; mode: c11.
 */
#include <stdio.h>
#define CHECK(x) do { if (!(x)) return 1; } while (0)


int main(void) {
int m=2,n=3,k=4; int a[m][n][k]; int (*p)[n][k]=a;
    a[1][2][3]=81; CHECK((p+1)-p==1 && sizeof *p==12*sizeof(int) && p[1][2][3]==81);
    puts("ok"); return 0;
}
