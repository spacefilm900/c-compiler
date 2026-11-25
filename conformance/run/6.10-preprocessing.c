/* N1570 6.10 preprocessing directives */
#include "check.h"
#include <string.h>
#include <stdint.h>
#define HDR <stddef.h>
#include HDR                                  /* 6.10.2p4: a macro-replaced #include */
#define QHDR "check.h"
#include QHDR

#if defined(__STDC__) && __STDC__ == 1 && __STDC_VERSION__ == 201112L && __STDC_HOSTED__ == 1
int predefined_ok = 1;
#else
int predefined_ok = 0;
#endif
#if -1 < 0u                                    /* 6.10.1p4: #if arithmetic uses intmax_t / uintmax_t */
#error "-1 < 0u should be false in #if"
#endif
#if 0x7fffffffffffffff + 0 != INTMAX_MAX || (0 ? 1/0 : 2) != 2 || 'A' != 65 || !defined UNDEF_X == 0
#error "if arithmetic"
#endif
#if UNDEFINED_NAME == 0 && (UNDEFINED_NAME + 1) == 1  /* unknown identifiers are 0 */
int undef_zero = 1;
#endif
#ifdef __STDC__
#elif garbage tokens ( are not evaluated in a skipped group
#endif
#if 0
#unknown_directive is not an error in a skipped group
#include <does/not/exist.h>
#else
int else_taken = 1;
#endif
#ifndef GUARD
#define GUARD
#elif 1
#error "elif after a true group"
#endif
#define OBJ (1 + 2)
#define FN(a, b) ((a) * (b))
#define STR(x) #x
#define XSTR(x) STR(x)
#define CAT(a, b) a ## b
#define XCAT(a, b) CAT(a, b)
#define VA(fmt, ...) snprintf(buf, sizeof buf, fmt, __VA_ARGS__)
#define VA0(...) #__VA_ARGS__
#define SELF SELF + 1                           /* 6.10.3.4p2: not replaced again */
#define f(a) a*g
#define g(a) f(a)
#define EMPTY
#define LPAREN (
#define F2(x, y) x + y
#define OBJ2 OBJ
#define OBJ   (1 + 2)                          /* identical redefinition is allowed */
#define FN(a, b) ((a) * (b))
#define AA(x) x
#define BB AA(
#define ID(x) x
#define HASH_HASH # ## #
#define mkstr(a) # a
#define in_between(a) mkstr(a)
#define join(c, d) in_between(c hash_hash d)
#define hash_hash # ## #
#line 500 "renamed.c"
static int line_after = __LINE__;
static const char *file_after = __FILE__;
#pragma STDC FP_CONTRACT ON
#pragma STDC FENV_ACCESS OFF
#pragma STDC CX_LIMITED_RANGE DEFAULT
#pragma some_unknown_pragma is ignored
#

int main(void) {
    char buf[64];
    CHECK(predefined_ok && undef_zero && else_taken);
    CHECK(OBJ == 3 && FN(1 + 1, 3) == 6);
    CHECK(strcmp(STR(a  +   "b\n"), "a + \"b\\n\"") == 0);   /* 6.10.3.2: whitespace and escaping */
    CHECK(strcmp(XSTR(OBJ), "(1 + 2)") == 0 && strcmp(STR(OBJ), "OBJ") == 0);
    CHECK(CAT(1, 2) == 12 && XCAT(O, BJ) == 3);
    int x12 = 7;
    CHECK(CAT(x, 12) == 7);
    VA("%d-%d", 1, 2);
    CHECK(strcmp(buf, "1-2") == 0);
    CHECK(strcmp(VA0(a, b , c), "a, b , c") == 0 && strcmp(VA0(), "") == 0);
    CHECK(strcmp(XSTR(SELF), "SELF + 1") == 0);
    CHECK(strcmp(XSTR(f(2)(9)), "2*9*g") == 0);       /* 6.10.3.5p5 example fragment */
    CHECK(F2(EMPTY, 2) == 2);
    int spans = BB 5);                             /* the invocation may span the macro's replacement */
    CHECK(spans == 5);
    CHECK(strcmp(join(x, y), "x ## y") == 0);       /* 6.10.3.3p4 example */
    CHECK(line_after == 500 && strcmp(file_after, "renamed.c") == 0);
    CHECK(__LINE__ > 500);
    CHECK(strlen(__DATE__) == 11 && strlen(__TIME__) == 8 && __TIME__[2] == ':');
    CHECK(sizeof(__FILE__) > 1);
    return DONE();
}
