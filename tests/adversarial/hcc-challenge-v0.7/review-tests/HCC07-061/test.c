/* HCC07-061: Aligned compound literal and record ABI across translation units
 * Basis: 6.7.5; 6.5.2.5; 6.5.2.2; 6.9.1
 * An aligned member changes record layout; passing and returning this type must preserve fields and alignment in mixed object files.
 * Expected: exit 0; stdout: ok; mode: c11.
 */
#include <stdio.h>
#define CHECK(x) do { if (!(x)) return 1; } while (0)
#include <stdint.h>
struct R { _Alignas(16) double value; int tag; };
extern struct R f(struct R);
int main(void) {
struct R r=f((struct R){.value=2.25,.tag=7}); CHECK(r.value==4.5 && r.tag==8 && (uintptr_t)&r%16==0);
    puts("ok"); return 0;
}
