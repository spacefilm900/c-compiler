/* HCC07-048: call_once publishes ordinary data to concurrent callers
 * Basis: 7.26.2.1
 * Completion of initialization synchronizes with return from other call_once calls.
 * Expected: exit 0; stdout: ok; mode: c11.
 */
#include <stdio.h>
#define CHECK(x) do { if (!(x)) return 1; } while (0)
#include <threads.h>
static once_flag f=ONCE_FLAG_INIT; static int val,count;
static void init(void){ val=42; count++; }
static int worker(void *p){ (void)p; call_once(&f,init); return val; }
int main(void) {
thrd_t a,b; int r=0,s=0; CHECK(thrd_create(&a,worker,0)==thrd_success); CHECK(thrd_create(&b,worker,0)==thrd_success); CHECK(thrd_join(a,&r)==thrd_success && thrd_join(b,&s)==thrd_success); CHECK(r==42 && s==42 && count==1);
    puts("ok"); return 0;
}
