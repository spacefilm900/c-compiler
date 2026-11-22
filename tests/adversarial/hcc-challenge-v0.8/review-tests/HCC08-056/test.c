/* HCC08-056: VLA loop with early return from nested scopes
 * 6.2.4; 6.5.3.4; 6.7.6.2; 6.7.6.3; 6.8.6. Defined size, indexing, lifetime or control-flow invariant. */
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#define CHECK(x) do { if (!(x)) return __LINE__; } while(0)
int work(int n){int a[n];a[0]=6;for(int i=0;i<n;i++){int b[n+i];b[0]=i;if(i==6)return a[0]*b[0]+6;}return 0;}
int main(void){
CHECK(work(7)==42);
puts("ok");return 0;
}
