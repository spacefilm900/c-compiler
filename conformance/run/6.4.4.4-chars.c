/* N1570 6.4.4.4 character constants */
#include "check.h"
#include <wchar.h>
#include <uchar.h>

#define T(e) _Generic((e), int: "int", unsigned: "uint", long: "long", unsigned short: "ushort", \
    unsigned long: "ulong", default: "?")

int main(void) {
    CHECK('a' == 97 && '\n' == 10 && '\0' == 0 && '\x41' == 'A' && '\101' == 'A');
    CHECK('\'' == 39 && '"' == 34 && '\"' == 34 && '\?' == 63 && '\\' == 92);
    printf("%s %s %s %s\n", T('a'), T(L'a'), T(u'a'), T(U'a'));
    CHECK(sizeof('a') == sizeof(int));
    printf("%d %d %d\n", '\xff', '\377', '\x7f');
    CHECK(L'\x1234' == 0x1234 && u'\u00e9' == 0xe9 && U'\U0001F600' == 0x1F600);
    CHECK(L'a' == 97);
    /* 6.4.4.4p10: a multi-character constant has an implementation-defined value */
    printf("%d %d\n", 'ab', 'abcd');
    return DONE();
}
