/* HCC08-031: Atomic pointer exchange with side-effecting new value. N1570 7.17.7.3. */
#include <stdatomic.h>
#include <stdio.h>
#define CHECK(x) do { if (!(x)) return __LINE__; } while(0)
int main(void){int a[6]={0};_Atomic(int*) p=a; int k=2; int *old=atomic_exchange_explicit(&p,a+k++,memory_order_acq_rel); CHECK(old==a && p==a+2 && k==3); puts("ok");return 0;}
