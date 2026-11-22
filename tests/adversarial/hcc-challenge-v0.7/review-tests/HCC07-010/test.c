/* HCC07-010: TLS extern across translation units
 * Basis: 6.7.1p3; 6.2.4p4
 * All declarations carry TLS; each thread has its own instance; thread starts with initializer.
 * Expected: exit 0; stdout: ok; mode: c11.
 */
#include <stdio.h>
#define CHECK(x) do { if (!(x)) return 1; } while (0)
#include <threads.h>
extern _Thread_local int g;
int worker(void *p) { (void)p; return ++g; }
int main(void) {
thrd_t t; int r; g=99; CHECK(thrd_create(&t,worker,0)==thrd_success); CHECK(thrd_join(t,&r)==thrd_success); CHECK(r==12 && g==99);
    puts("ok"); return 0;
}
