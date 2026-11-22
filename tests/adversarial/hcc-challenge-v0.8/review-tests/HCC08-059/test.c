/* HCC08-059: VLA and function pointer with runtime stride
 * 6.2.4; 6.5.3.4; 6.7.6.2; 6.7.6.3; 6.8.6. Defined size, indexing, lifetime or control-flow invariant. */
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#define CHECK(x) do { if (!(x)) return __LINE__; } while(0)
int work(int n,int (*a)[n]){a[1][n-1]=53;return a[1][n-1];}
int main(void){
int n=5;int a[2][n];int (*f)(int,int (*)[n])=work;CHECK(f(n,a)==53);
puts("ok");return 0;
}
