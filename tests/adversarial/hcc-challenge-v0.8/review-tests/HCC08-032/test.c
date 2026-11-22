/* HCC08-032: Atomic pointer CAS with typed expected
 * 6.5.16.2; 7.17.3; 7.17.7; 7.17.8. Defined operation and exact state checks; no padding comparison. */
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#define CHECK(x) do { if (!(x)) return __LINE__; } while(0)
#include <stdatomic.h>

int main(void){
int a[3]={0}; _Atomic(int*) p=a; int *e=a+1; CHECK(!atomic_compare_exchange_strong(&p,&e,a+2)); CHECK(e==a); CHECK(atomic_compare_exchange_strong(&p,&e,a+2)); CHECK(p==a+2);
puts("ok");return 0;
}
