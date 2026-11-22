/* HCC08-060: VLA pointer casts with runtime row type
 * 6.2.4; 6.5.3.4; 6.7.6.2; 6.7.6.3; 6.8.6. Defined size, indexing, lifetime or control-flow invariant. */
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#define CHECK(x) do { if (!(x)) return __LINE__; } while(0)

int main(void){
int n=4;int a[2][n];void *v=a;int (*p)[n]=v;p[1][2]=51;CHECK(a[1][2]==51);
puts("ok");return 0;
}
