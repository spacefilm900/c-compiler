/* HCC07-050: Release/acquire publishes non-atomic payload
 * Basis: 5.1.2.4; 7.17.3; 7.26.5
 * A successful acquire load reading the release store synchronizes payload access; no data race.
 * Expected: exit 0; stdout: ok; mode: c11.
 */
#include <stdio.h>
#define CHECK(x) do { if (!(x)) return 1; } while (0)
#include <threads.h>
#include <stdatomic.h>
static atomic_int ready=0; static int payload;
static int reader(void *p){ (void)p; while(!atomic_load_explicit(&ready,memory_order_acquire)) thrd_yield(); return payload; }
static int writer(void *p){(void)p; payload=42; atomic_store_explicit(&ready,1,memory_order_release); return 0;}
int main(void) {
thrd_t a,b; int r,s; CHECK(thrd_create(&a,reader,0)==thrd_success); CHECK(thrd_create(&b,writer,0)==thrd_success); CHECK(thrd_join(a,&r)==thrd_success && thrd_join(b,&s)==thrd_success); CHECK(r==42 && s==0);
    puts("ok"); return 0;
}
