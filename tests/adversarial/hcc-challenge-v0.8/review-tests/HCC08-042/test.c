/* HCC08-042: Atomic floating multiply contended
 * 5.1.2.4; 6.2.4; 7.17; 7.26. Deterministic invariant after synchronisation; no timing-dependent expected value. */
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#define CHECK(x) do { if (!(x)) return __LINE__; } while(0)
#include <stdatomic.h>
#include <threads.h>
_Atomic(double) x=1.0; int worker(void*p){(void)p;for(int i=0;i<8;i++)x*=2.0;return 0;}
int main(void){
thrd_t t[3];for(int i=0;i<3;i++)CHECK(thrd_create(&t[i],worker,0)==thrd_success);for(int i=0;i<3;i++)CHECK(thrd_join(t[i],0)==thrd_success);CHECK(x==16777216.0);
puts("ok");return 0;
}
