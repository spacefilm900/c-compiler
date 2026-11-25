/* N1570 6.10.3.5 the macro replacement examples (EXAMPLE 3 to 7), stringised */
#include <stdio.h>
#define S(...) #__VA_ARGS__
#define XS(...) S(__VA_ARGS__)

/* EXAMPLE 3 */
#define x 3
#define f(a) f(x * (a))
#undef x
#define x 2
#define g f
#define z z[0]
#define h g(~
#define m(a) a(w)
#define w 0,1
#define t(a) a
#define p() int
#define q(x) x
#define r(x,y) x ## y
#define str(x) # x
static const char *ex3[] = {
    XS(f(y+1) + f(f(z)) % t(t(g)(0) + t)(1);),
    /* The second line, g(x+(3,4)-w) | h 5) & m (f)^m(m);, cannot be an
       argument (h's parenthesis is unbalanced); src/pp/tests.rs checks it. */
    XS(f(2)(9)),
    XS(p() i[q()] = { q(1), r(2,3), r(4,), r(,5), r(,) };),
    XS(char c[2][6] = { str(hello), str() };),
};
#undef f
#undef g
#undef h
#undef m
#undef p
#undef q
#undef r
#undef t
#undef w
#undef x
#undef z
#undef str

/* EXAMPLE 4 */
#define str(s) # s
#define xstr(s) str(s)
#define debug(s, t) printf("x" # s "= %d, x" # t "= %s", \
        x ## s, x ## t)
#define INCFILE(n) vers ## n
#define glue(a, b) a ## b
#define xglue(a, b) glue(a, b)
#define HIGHLOW "hello"
#define LOW LOW ", world"
static const char *ex4[] = {
    XS(debug(1, 2);),
    XS(fputs(str(strncmp("abc\0d", "abc", '\4') // this goes away
        == 0) str(: @\n), s);),
    XS(xstr(INCFILE(2).h)),
    XS(glue(HIGH, LOW);),
    XS(xglue(HIGH, LOW)),
};
#undef str
#undef xstr
#undef debug

/* EXAMPLE 5 */
#define t(x,y,z) x ## y ## z
static const char *ex5 = XS(int j[] = { t(1,2,3), t(,4,5), t(6,,7), t(8,9,),
    t(10,,), t(,11,), t(,,12), t(,,) };);
#undef t

/* EXAMPLE 7 */
#define debug(...) fprintf(stderr, __VA_ARGS__)
#define showlist(...) puts(#__VA_ARGS__)
#define report(test, ...) ((test)?puts(#test):\
    printf(__VA_ARGS__))
static const char *ex7[] = {
    XS(debug("Flag");),
    XS(debug("X = %d\n", x);),
    XS(showlist(The first, second, and third items.);),
    XS(report(x>y, "x is %d but y is %d", x, y);),
};

/* 6.10.3.3p4 EXAMPLE and 6.10.3.4p4 EXAMPLE */
#define hash_hash # ## #
#define mkstr(a) # a
#define in_between(a) mkstr(a)
#define join(c, d) in_between(c hash_hash d)
#define OBJ_LIKE (1-1)
#define OBJ_LIKE /* white space */ (1-1) /* other */
#define FUNC_LIKE(a) ( a )
#define FUNC_LIKE( a )( /* note the white space */ \
        a /* other stuff on this line
        */ )

int main(void) {
    for (unsigned i = 0; i < sizeof ex3 / sizeof *ex3; i++) puts(ex3[i]);
    for (unsigned i = 0; i < sizeof ex4 / sizeof *ex4; i++) puts(ex4[i]);
    puts(ex5);
    for (unsigned i = 0; i < sizeof ex7 / sizeof *ex7; i++) puts(ex7[i]);
    puts(join(x, y));
    puts(XS(OBJ_LIKE FUNC_LIKE(z)));
    return 0;
}
