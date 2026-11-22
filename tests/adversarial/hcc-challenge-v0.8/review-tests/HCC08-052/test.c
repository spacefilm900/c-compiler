/* HCC08-052: VLA parameter captures bound and modifies independently
 * 6.2.4; 6.5.3.4; 6.7.6.2; 6.7.6.3; 6.8.6. Defined size, indexing, lifetime or control-flow invariant. */
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#define CHECK(x) do { if (!(x)) return __LINE__; } while(0)
int fill(int n,int a[][n]){n+=3;a[1][3]=31;return sizeof *a==4*sizeof(int)?a[1][3]:0;}
int main(void){
int n=4;int a[2][n];CHECK(fill(n,a)==31 && a[1][3]==31);
puts("ok");return 0;
}
