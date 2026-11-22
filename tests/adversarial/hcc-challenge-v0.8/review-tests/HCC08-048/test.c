/* HCC08-048: Thread exit publishes return code and TLS
 * 5.1.2.4; 6.2.4; 7.17; 7.26. Deterministic invariant after synchronisation; no timing-dependent expected value. */
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#define CHECK(x) do { if (!(x)) return __LINE__; } while(0)
#include <stdatomic.h>
#include <threads.h>
_Thread_local int local; int worker(void*p){local=*(int*)p;thrd_exit(local+5);return 99;}
int main(void){
int n=26,v=0;thrd_t t;local=71;CHECK(thrd_create(&t,worker,&n)==thrd_success);CHECK(thrd_join(t,&v)==thrd_success&&v==31);CHECK(local==71);
puts("ok");return 0;
}
