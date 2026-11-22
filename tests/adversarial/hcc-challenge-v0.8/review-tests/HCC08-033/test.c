/* HCC08-033: Unsigned bitwise fetch sequence
 * 6.5.16.2; 7.17.3; 7.17.7; 7.17.8. Defined operation and exact state checks; no padding comparison. */
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#define CHECK(x) do { if (!(x)) return __LINE__; } while(0)
#include <stdatomic.h>

int main(void){
atomic_uint x=ATOMIC_VAR_INIT(0x35u); CHECK(atomic_fetch_xor(&x,0x12u)==0x35u); CHECK(atomic_fetch_or(&x,0x40u)==0x27u); CHECK(atomic_fetch_and(&x,0x33u)==0x67u); CHECK(x==0x23u);
puts("ok");return 0;
}
