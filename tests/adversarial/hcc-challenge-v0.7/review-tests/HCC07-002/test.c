/* HCC07-002: Multiple specifiers with weaker and zero
 * Basis: 6.7.5p4,p6
 * Only the combined alignment must meet the natural alignment; zero imposes none.
 * Expected: exit 0; stdout: ok; mode: c11.
 */
#include <stdio.h>
#define CHECK(x) do { if (!(x)) return 1; } while (0)
#include <stdint.h>

int main(void) {
_Alignas(1) _Alignas(0) _Alignas(16) double x=1.5;
    CHECK((uintptr_t)&x%16==0 && x==1.5);
    puts("ok"); return 0;
}
