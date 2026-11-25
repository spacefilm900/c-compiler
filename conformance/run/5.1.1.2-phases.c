/* N1570 5.1.1.2 translation phases */
#include "check.h"
#include <string.h>
#include <wchar.h>

int spl\
iced = 7;   /* phase 2: a backslash-newline joins lines, even inside an identifier */
#define TWO 1 + \
 1

/* phase 4: _Pragma is processed */
_Pragma("STDC FP_CONTRACT OFF")

int main(void) {
    CHECK(spliced == 7);
    CHECK(TWO == 2);
    /* phase 1: trigraphs (C11 5.2.1.1) */
    CHECK(strcmp("??=??(??)??<??>??/??/??'??!??-", "#[]{}\\^|~") == 0);
    /* phase 3: a comment is one space */
    int c/**/= 1;
    CHECK(c == 1);
    CHECK(sizeof "x/**/y" == 7);   /* comments are not recognised in string literals */
    /* phase 6: adjacent string literals concatenate, a prefix applies to all */
    CHECK(strcmp("ab" "cd", "abcd") == 0);
    CHECK(sizeof("a" L"b") / sizeof(wchar_t) == 3);
    CHECK(sizeof("\x12" "3") == 3);   /* escapes are converted before concatenation */
    return DONE();
}
