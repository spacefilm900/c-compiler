/* HCC07-049: Thread exit and TSS destructor iteration
 * Basis: 7.26.5.5; 7.26.6
 * Destructor resets its own key twice; third call clears it; thread completion synchronizes with join.
 * Expected: exit 0; stdout: ok; mode: c11.
 */
#include <stdio.h>
#define CHECK(x) do { if (!(x)) return 1; } while (0)
#include <threads.h>
static tss_t key; static int count,error,token;
static void dtor(void *p){ if(p!=&token) error=1; ++count; if(count<3 && tss_set(key,&token)!=thrd_success) error=1; }
static int worker(void *p){ (void)p; if(tss_set(key,&token)!=thrd_success) return -1; thrd_exit(13); return -2; }
int main(void) {
thrd_t t; int r; CHECK(tss_create(&key,dtor)==thrd_success); CHECK(thrd_create(&t,worker,0)==thrd_success); CHECK(thrd_join(t,&r)==thrd_success); CHECK(r==13 && count==3 && !error); tss_delete(key);
    puts("ok"); return 0;
}
