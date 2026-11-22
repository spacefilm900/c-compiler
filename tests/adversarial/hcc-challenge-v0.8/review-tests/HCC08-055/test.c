/* HCC08-055: VLA multidimensional compound assignment pointer
 * 6.2.4; 6.5.3.4; 6.7.6.2; 6.7.6.3; 6.8.6. Defined size, indexing, lifetime or control-flow invariant. */
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#define CHECK(x) do { if (!(x)) return __LINE__; } while(0)

int main(void){
int n=4;int a[4][n];int (*p)[n]=a;p+=2;(*p)[3]=19;p--;(*p)[1]=29;CHECK(a[2][3]==19&&a[1][1]==29);
puts("ok");return 0;
}
