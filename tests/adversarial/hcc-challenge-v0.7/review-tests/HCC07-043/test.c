/* HCC07-043: VLA of atomic members and alignment
 * Basis: 6.7.6.2; 6.7.3; 7.17.7.1
 * An array of atomic-qualified elements is distinct from atomic array type specifier.
 * Expected: exit 0; stdout: ok; mode: c11.
 */
#include <stdio.h>
#define CHECK(x) do { if (!(x)) return 1; } while (0)
#include <stdatomic.h>

int main(void) {
int n=5; _Atomic int a[n]; for(int i=0;i<n;i++) atomic_init(&a[i],i); for(int i=0;i<n;i++) a[i]+=2; CHECK(atomic_load(&a[4])==6 && sizeof a==n*sizeof(a[0]));
    puts("ok"); return 0;
}
