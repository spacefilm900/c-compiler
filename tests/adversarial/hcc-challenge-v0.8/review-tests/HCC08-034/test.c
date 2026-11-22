/* HCC08-034: Nonconstant order expression evaluated once
 * 6.5.16.2; 7.17.3; 7.17.7; 7.17.8. Defined operation and exact state checks; no padding comparison. */
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#define CHECK(x) do { if (!(x)) return __LINE__; } while(0)
#include <stdatomic.h>

int main(void){
atomic_int x=ATOMIC_VAR_INIT(2); memory_order m[2]={memory_order_relaxed,memory_order_seq_cst}; int k=0; CHECK(atomic_fetch_add_explicit(&x,5,m[k++])==2); CHECK(k==1 && x==7);
puts("ok");return 0;
}
