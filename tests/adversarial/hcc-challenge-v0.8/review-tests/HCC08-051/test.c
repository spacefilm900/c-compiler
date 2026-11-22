/* HCC08-051: VLA pointer difference after bound changes
 * 6.2.4; 6.5.3.4; 6.7.6.2; 6.7.6.3; 6.8.6. Defined size, indexing, lifetime or control-flow invariant. */
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#define CHECK(x) do { if (!(x)) return __LINE__; } while(0)

int main(void){
int n=4;int a[3][n];int (*p)[n]=a;n=7;p+=2;CHECK(p-a==2);CHECK(sizeof *p==4*sizeof(int));(*p)[3]=17;CHECK(a[2][3]==17);
puts("ok");return 0;
}
