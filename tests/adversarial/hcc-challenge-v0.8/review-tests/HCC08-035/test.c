/* HCC08-035: Atomic floating multiply and division
 * 6.5.16.2; 7.17.3; 7.17.7; 7.17.8. Defined operation and exact state checks; no padding comparison. */
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#define CHECK(x) do { if (!(x)) return __LINE__; } while(0)
#include <stdatomic.h>

int main(void){
_Atomic(double) x=8.0; double v=(x*=2.5); CHECK(v==20.0 && x==20.0); v=(x/=4.0); CHECK(v==5.0 && x==5.0);
puts("ok");return 0;
}
