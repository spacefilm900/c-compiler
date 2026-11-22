/* HCC08-039: Atomic elements in a VLA
 * 6.5.16.2; 7.17.3; 7.17.7; 7.17.8. Defined operation and exact state checks; no padding comparison. */
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#define CHECK(x) do { if (!(x)) return __LINE__; } while(0)
#include <stdatomic.h>

int main(void){
int n=7; atomic_int a[n]; for(int i=0;i<n;i++)atomic_init(&a[i],i); for(int i=0;i<n;i++) CHECK(atomic_fetch_add(&a[i],10)==i); for(int i=0;i<n;i++)CHECK(atomic_load(&a[i])==i+10);
puts("ok");return 0;
}
