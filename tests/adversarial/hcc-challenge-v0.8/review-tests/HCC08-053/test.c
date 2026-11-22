/* HCC08-053: VLA sizeof type evaluates dimension once
 * 6.2.4; 6.5.3.4; 6.7.6.2; 6.7.6.3; 6.8.6. Defined size, indexing, lifetime or control-flow invariant. */
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#define CHECK(x) do { if (!(x)) return __LINE__; } while(0)

int main(void){
int n=3;size_t z=sizeof(int[2][n++]);CHECK(z==6*sizeof(int)&&n==4);
puts("ok");return 0;
}
