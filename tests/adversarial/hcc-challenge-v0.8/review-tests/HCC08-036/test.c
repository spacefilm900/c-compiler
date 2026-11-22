/* HCC08-036: Atomic signed short compound operations
 * 6.5.16.2; 7.17.3; 7.17.7; 7.17.8. Defined operation and exact state checks; no padding comparison. */
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#define CHECK(x) do { if (!(x)) return __LINE__; } while(0)
#include <stdatomic.h>

int main(void){
_Atomic(short) x=12; CHECK((x*=3)==36); CHECK((x%=7)==1); CHECK((x<<=3)==8); CHECK(x--==8 && x==7);
puts("ok");return 0;
}
