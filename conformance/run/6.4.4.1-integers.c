/* N1570 6.4.4.1 integer constants: the type of each constant */
#include "check.h"

#define T(e) _Generic((e), int: "int", unsigned: "uint", long: "long", unsigned long: "ulong", \
    long long: "llong", unsigned long long: "ullong", default: "?")

int main(void) {
    printf("%s %s %s %s\n", T(2147483647), T(2147483648), T(4294967295), T(9223372036854775807));
    printf("%s %s %s %s\n", T(0x7fffffff), T(0x80000000), T(0xffffffff), T(0x100000000));
    printf("%s %s %s\n", T(0x7fffffffffffffff), T(0x8000000000000000), T(017777777777));
    printf("%s %s %s %s %s\n", T(1u), T(1U), T(1l), T(1L), T(1ul));
    printf("%s %s %s %s %s %s\n", T(1lu), T(1ll), T(1LL), T(1ull), T(1LLU), T(1uLL));
    printf("%s %s\n", T(4294967296u), T(0xffffffffffffffffu));
    CHECK(0x1F == 31 && 0X1f == 31 && 017 == 15 && 0 == 00);
    CHECK(-2147483648 == -2147483647 - 1);
    return DONE();
}
