/* HCC08-050: VLA typedef shadowing and capture
 * 6.2.4; 6.5.3.4; 6.7.6.2; 6.7.6.3; 6.8.6. Defined size, indexing, lifetime or control-flow invariant. */
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#define CHECK(x) do { if (!(x)) return __LINE__; } while(0)

int main(void){
int n=3;typedef int Row[n];Row a;{int n=7;typedef int Row[n];Row b;CHECK(sizeof b==7*sizeof(int));}n=9;CHECK(sizeof a==3*sizeof(int));CHECK(sizeof(Row)==3*sizeof(int));
puts("ok");return 0;
}
