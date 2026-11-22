/* HCC07-003: Aligned union and enclosing record copy
 * Basis: 6.7.5; 6.7.2.1; 6.5.16.1
 * Union member alignment propagates through enclosing record and array; copy preserves values.
 * Expected: exit 0; stdout: ok; mode: c11.
 */
#include <stdio.h>
#define CHECK(x) do { if (!(x)) return 1; } while (0)
#include <stdint.h>
union U { _Alignas(16) int x; double d; };
struct S { char tag; union U u; int z; };
int main(void) {
struct S a[2]; struct S b;
    a[1].u.x=17; a[1].z=9; b=a[1];
    CHECK(_Alignof(union U)>=16 && (uintptr_t)&a[1].u%16==0 && b.u.x==17 && b.z==9);
    puts("ok"); return 0;
}
