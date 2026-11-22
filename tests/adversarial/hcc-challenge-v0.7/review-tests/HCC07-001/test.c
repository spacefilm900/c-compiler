/* HCC07-001: Over-aligned nested record arrays
 * Basis: 6.2.8; 6.7.5; 6.7.2.1
 * Each element and each nested member must honor alignment; stride must preserve it. Extended alignment 32 is a documented target choice.
 * Expected: exit 0; stdout: ok; mode: c11.
 */
#include <stdio.h>
#define CHECK(x) do { if (!(x)) return 1; } while (0)
#include <stdint.h>
struct Inner { char tag; _Alignas(32) int value; };
struct Outer { char tag; struct Inner inner; char end; };
int main(void) {
struct Outer a[3];
    for (int i=0;i<3;++i) { a[i].inner.value=i+7; CHECK((uintptr_t)&a[i].inner.value % 32 == 0); }
    CHECK(_Alignof(struct Outer) >= 32 && sizeof(struct Outer)%32 == 0);
    CHECK(a[2].inner.value == 9);
    puts("ok"); return 0;
}
