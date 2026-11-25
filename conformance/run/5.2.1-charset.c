/* N1570 5.2.1 character sets and 5.2.2 character display semantics */
#include "check.h"
#include <string.h>

int main(void) {
    const char *digits = "0123456789";
    for (int i = 0; i < 10; i++)
        CHECK(digits[i] - '0' == i);   /* 5.2.1p3: digits are contiguous */
    const char basic[] = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789!\"#%&'()*+,-./:;<=>?[\\]^_{|}~ \t\v\f";
    CHECK(sizeof basic == 26 + 26 + 10 + 29 + 4 + 1);
    CHECK('\0' == 0);
    CHECK(strlen("a\0b") == 1);
    CHECK('\a' != 0 && '\b' != 0 && '\f' != 0 && '\n' != 0 && '\r' != 0 && '\t' != 0 && '\v' != 0);
    printf("%d %d %d %d %d %d %d\n", '\a', '\b', '\f', '\n', '\r', '\t', '\v');
    return DONE();
}
