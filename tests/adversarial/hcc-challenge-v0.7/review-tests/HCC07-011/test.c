/* HCC07-011: Aligned TLS inside a worker
 * Basis: 6.2.4p4; 6.7.5; 7.26.5
 * Local static TLS persists per thread across calls and honors alignment.
 * Expected: exit 0; stdout: ok; mode: c11.
 */
#include <stdio.h>
#define CHECK(x) do { if (!(x)) return 1; } while (0)
#include <stdint.h>
#include <threads.h>
static int bump(void) { static _Thread_local _Alignas(16) int n=4; if ((uintptr_t)&n%16) return -100; return ++n; }
static int worker(void *p) { (void)p; int a=bump(); return 10*a+bump(); }
int main(void) {
thrd_t t; int r; CHECK(bump()==5); CHECK(thrd_create(&t,worker,0)==thrd_success); CHECK(thrd_join(t,&r)==thrd_success); CHECK(r==56 && bump()==6);
    puts("ok"); return 0;
}
