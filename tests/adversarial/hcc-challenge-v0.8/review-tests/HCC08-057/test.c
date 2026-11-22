/* HCC08-057: VLA goto backward reevaluates bound
 * 6.2.4; 6.5.3.4; 6.7.6.2; 6.7.6.3; 6.8.6. Defined size, indexing, lifetime or control-flow invariant. */
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#define CHECK(x) do { if (!(x)) return __LINE__; } while(0)

int main(void){
int n=2,k=0,total=0;again: ; int a[n++];a[0]=k;total+=(int)(sizeof a/sizeof(int));if(++k<5)goto again;CHECK(total==20&&n==7);
puts("ok");return 0;
}
