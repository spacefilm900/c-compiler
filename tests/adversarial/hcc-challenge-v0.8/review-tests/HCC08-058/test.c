/* HCC08-058: VLA nested switch exits with outer storage intact
 * 6.2.4; 6.5.3.4; 6.7.6.2; 6.7.6.3; 6.8.6. Defined size, indexing, lifetime or control-flow invariant. */
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#define CHECK(x) do { if (!(x)) return __LINE__; } while(0)

int main(void){
int n=3;int a[n];a[0]=41;for(int k=0;k<20;k++){switch(k%2){case 0:{int b[n+k];b[0]=k;break;}default:{int c[n+k];c[0]=k;continue;}}CHECK(a[0]==41);}CHECK(a[0]==41);
puts("ok");return 0;
}
