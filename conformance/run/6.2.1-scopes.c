/* N1570 6.2.1 scopes of identifiers, 6.2.3 name spaces */
#include "check.h"

int x = 1;
struct tag { int tag; } tag;   /* 6.2.3: tag, member and ordinary names are separate */
enum { E = 5 };

int f(int x, int (*g)(int x)) { return x + (g ? g(x) : 0); }   /* prototype scope */
static int twice(int v) { return 2 * v; }

int main(void) {
    CHECK(x == 1);
    int x = 2;                       /* hides the file-scope x */
    CHECK(x == 2);
    {
        extern int x;                /* 6.2.2p4: refers to the file-scope object */
        CHECK(x == 1);
    }
    {
        int x[3] = { sizeof x / sizeof x[0] };  /* 6.2.1p7: scope starts after the declarator */
        CHECK(x[0] == 3);
    }
    tag.tag = 4;
    CHECK(tag.tag == 4);
    goto tag;                        /* labels have function scope */
tag:
    CHECK(f(1, twice) == 3);
    enum { E = E + 1 };              /* 6.2.1p7: an enumerator's scope starts after it */
    CHECK(E == 6);
    for (int x = 10; x < 11; x++)    /* 6.8.5p5: the for loop is a block */
        CHECK(x == 10);
    CHECK(x == 2);
    if (sizeof(struct inner { int a; }) == sizeof(int)) {  /* 6.8.4p3: selection statements are blocks */
        struct inner i = { 1 };
        CHECK(i.a == 1);
    }
    return DONE();
}
