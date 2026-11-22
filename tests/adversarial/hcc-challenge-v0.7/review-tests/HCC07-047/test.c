/* HCC07-047: Concurrent atomic increment with exact total
 * Basis: 6.5.2.4p2; 7.17; 7.26.5
 * Two workers perform atomic RMW; join makes final observation deterministic.
 * Expected: exit 0; stdout: ok; mode: c11.
 */
#include <stdio.h>
#define CHECK(x) do { if (!(x)) return 1; } while (0)
#include <threads.h>
#include <stdatomic.h>
static atomic_int counter=0;
static int worker(void *p){ (void)p; for (int i=0;i<5000;i++) ++counter; return 0; }
int main(void) {
thrd_t a,b; int r,s; CHECK(thrd_create(&a,worker,0)==thrd_success); CHECK(thrd_create(&b,worker,0)==thrd_success); CHECK(thrd_join(a,&r)==thrd_success && thrd_join(b,&s)==thrd_success); CHECK(r==0 && s==0 && atomic_load(&counter)==10000);
    puts("ok"); return 0;
}
