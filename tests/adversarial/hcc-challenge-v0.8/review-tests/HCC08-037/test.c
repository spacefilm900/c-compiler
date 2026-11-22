/* HCC08-037: Atomic aggregate exchange without padding
 * 6.5.16.2; 7.17.3; 7.17.7; 7.17.8. Defined operation and exact state checks; no padding comparison. */
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#define CHECK(x) do { if (!(x)) return __LINE__; } while(0)
#include <stdatomic.h>

int main(void){
struct Pair {unsigned a,b;}; _Atomic(struct Pair) x; struct Pair p={2,7},q={9,11}; atomic_init(&x,p); struct Pair old=atomic_exchange(&x,q); struct Pair now=atomic_load(&x); CHECK(old.a==2 && old.b==7 && now.a==9 && now.b==11);
puts("ok");return 0;
}
