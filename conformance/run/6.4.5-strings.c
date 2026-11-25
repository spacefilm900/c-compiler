/* N1570 6.4.5 string literals */
#include "check.h"
#include <string.h>
#include <wchar.h>
#include <uchar.h>

int main(void) {
    CHECK(sizeof "abc" == 4 && sizeof "" == 1 && sizeof "a\0b" == 4);
    CHECK(_Generic("x", char *: 1, default: 0));
    CHECK(_Generic(L"x", wchar_t *: 1, default: 0));
    CHECK(_Generic(u"x", char16_t *: 1, default: 0) && _Generic(U"x", char32_t *: 1, default: 0));
    CHECK(_Generic(u8"x", char *: 1, default: 0));
    CHECK(sizeof u8"\u00e9" == 3 && (unsigned char)u8"\u00e9"[0] == 0xc3);
    CHECK(sizeof u"\U0001F600" == 3 * sizeof(char16_t));   /* a surrogate pair */
    CHECK(sizeof U"\U0001F600" == 2 * sizeof(char32_t));
    CHECK(sizeof L"ab" == 3 * sizeof(wchar_t));
    CHECK(sizeof("a" u8"b") == 3 && sizeof(L"a" "b") == 3 * sizeof(wchar_t) && sizeof(u"a" "b") == 3 * sizeof(char16_t));
    CHECK(strcmp("\x41" "B", "AB") == 0);
    CHECK(sizeof "\1234" == 3);          /* an octal escape is at most three digits */
    const char *p = "same", *q = "same";
    (void)(p == q);                       /* unspecified whether distinct */
    char a[] = "xyz";
    a[0] = 'X';                           /* an array initialised from a literal is modifiable */
    CHECK(strcmp(a, "Xyz") == 0);
    return DONE();
}
