/* HCC07-023: Ternary real precision and integer argument
 * Basis: 7.25p3
 * All generic arguments participate in selection, including integer mapped to double.
 * Expected: exit 0; stdout: ok; mode: c11.
 */
#include <stdio.h>
#define CHECK(x) do { if (!(x)) return 1; } while (0)
#include <tgmath.h>

int main(void) {
CHECK(_Generic(fma(1.0f,2.0f,3),double:1,default:0));
    CHECK(_Generic(fma(1.0f,2.0L,3.0),long double:1,default:0));
    CHECK(fma(2.0f,3.0L,4.0f)==10.0L);
    puts("ok"); return 0;
}
