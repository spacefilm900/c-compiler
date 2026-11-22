/* HCC07-063: Empty initializer for fixed-size record and array
 * Basis: N1570 6.7.9; GCC manual C extensions
 * GNU empty initializer zero-initializes named fields and all fixed-size elements; incomplete array case is deliberately not tested.
 * Expected: exit 0; stdout: ok; mode: gnu11.
 */
#include <stdio.h>
#define CHECK(x) do { if (!(x)) return 1; } while (0)

struct S { int x; double y; };
int main(void) {
struct S s={}; int a[3]={}; CHECK(s.x==0 && s.y==0.0 && a[0]==0 && a[2]==0);
    puts("ok"); return 0;
}
