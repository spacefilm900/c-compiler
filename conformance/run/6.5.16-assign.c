/* N1570 6.5.16 assignment operators, 6.5.17 comma operator, 6.5p2 sequencing */
#include "check.h"

struct big { int v[8]; };

int main(void) {
    int a, b, c;
    a = b = c = 3;                             /* right associative */
    CHECK(a == 3 && b == 3 && c == 3);
    char ch;
    CHECK(_Generic(ch = 300, char: 1, default: 0));   /* the result has the type of the left operand */
    CHECK((ch = 'A') == 'A');
    unsigned char uc;
    CHECK((uc = 511) == 255);
    int x = 10;
    x += 5; CHECK(x == 15);
    x -= 3; CHECK(x == 12);
    x *= 2; CHECK(x == 24);
    x /= 5; CHECK(x == 4);
    x %= 3; CHECK(x == 1);
    x <<= 4; CHECK(x == 16);
    x >>= 2; CHECK(x == 4);
    x &= 6; CHECK(x == 4);
    x |= 3; CHECK(x == 7);
    x ^= 5; CHECK(x == 2);
    double d = 1;
    d += 0.5; CHECK(d == 1.5);
    int i = 7;
    i *= 0.5;                                  /* computed in double, then converted */
    CHECK(i == 3);
    int arr[4] = { 0 }, *p = arr;
    p += 2; CHECK(p == arr + 2);
    p -= 1; CHECK(p == arr + 1);
    struct big s1 = { { 1, 2, 3 } }, s2;
    s2 = s1;
    CHECK(s2.v[2] == 3);
    _Bool bo;
    bo = 7; CHECK(bo == 1);
    bo = 0.1; CHECK(bo == 1);
    /* 6.5.17: comma */
    int k = (a = 1, b = a + 1, b * 10);
    CHECK(k == 20);
    CHECK(_Generic((0, 1.0f), float: 1, default: 0));
    for (a = 0, b = 10; a < b; a++, b--) ;
    CHECK(a == 5);
    return DONE();
}
