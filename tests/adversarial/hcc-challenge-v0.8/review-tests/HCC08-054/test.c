/* HCC08-054: VLA typedef declaration side effect once
 * 6.2.4; 6.5.3.4; 6.7.6.2; 6.7.6.3; 6.8.6. Defined size, indexing, lifetime or control-flow invariant. */
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#define CHECK(x) do { if (!(x)) return __LINE__; } while(0)

int main(void){
int n=3;typedef int T[n++];T a,b;CHECK(n==4);CHECK(sizeof a==3*sizeof(int)&&sizeof b==sizeof(T));
puts("ok");return 0;
}
