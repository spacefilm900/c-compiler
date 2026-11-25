/* N1570 6.4.4.2 floating constants */
#include "check.h"

#define T(e) _Generic((e), float: "float", double: "double", long double: "ldouble", default: "?")

int main(void) {
    printf("%s %s %s %s\n", T(1.0), T(1.0f), T(1.0F), T(1.0L));
    printf("%s %s %s\n", T(1e3), T(.5), T(5.));
    CHECK(0x1p4 == 16.0 && 0x1.8p1 == 3.0 && 0X.8P0 == 0.5 && 0x1P-2f == 0.25f);
    CHECK(1e3 == 1000.0 && 1E-3 == 0.001 && 5. == 5.0 && .5 == 0.5);
    CHECK(0.1f == (float)0.1);
    printf("%a %a %La %.17g\n", 0.1, 0x1.fffffffffffffp1023, 0.1L, 1e-320);
    printf("%a %a\n", 123456789012345678901234567890.0, 0x1.000000000000081p0);
    return DONE();
}
