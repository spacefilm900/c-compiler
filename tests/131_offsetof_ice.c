/* offsetof expands to __builtin_offsetof, an integer constant
   expression of type size_t (C11 7.19p3, 6.6p6). The member designator
   may name nested members, anonymous members, and array elements with
   constant subscripts. Every folded value must equal the address
   difference measured at run time. A subscript that is not constant
   (a GNU extension) is evaluated at run time. No headers: the builtin
   is used directly. */

typedef unsigned long size_t;

struct Leaf {
    char tag;
    double weight;
    short code[4];
};

struct Tree {
    int id;
    struct Leaf leaves[5];
    union {
        char c;
        long l;
    } u;
    struct {
        char x;
        int y;
    } pair;
    struct {
        char a;
        long long b;
    };
};

union Mixed {
    int i;
    char buf[12];
};

enum { TWO = 2 };

_Static_assert(__builtin_offsetof(struct Tree, id) == 0, "first member");
_Static_assert(__builtin_offsetof(struct Tree, leaves[2].code[3])
                   > __builtin_offsetof(struct Tree, leaves[2]),
               "nested array element");
_Static_assert(__builtin_offsetof(union Mixed, buf[7]) == 7, "union member");

enum {
    LEAF3 = __builtin_offsetof(struct Tree, leaves[3]),
    PAIR_Y = __builtin_offsetof(struct Tree, pair.y),
    ANON_B = __builtin_offsetof(struct Tree, b),
};

static const size_t code_off = __builtin_offsetof(struct Tree, leaves[1 + 1].code[TWO * 1]);
static size_t table[] = {
    __builtin_offsetof(struct Tree, u),
    __builtin_offsetof(struct Tree, u.l),
    __builtin_offsetof(struct Leaf, code[0]),
    __builtin_offsetof(struct Tree, leaves[sizeof(char)].weight),
};
static char bound[__builtin_offsetof(struct Tree, pair) + 1];

static size_t diff(void *member, void *base) {
    return (size_t)((char *)member - (char *)base);
}

int main(void) {
    struct Tree t;
    union Mixed m;
    volatile int i = 4;

    if (diff(&t.leaves[2].code[2], &t) != code_off)
        return 1;
    if (diff(&t.leaves[3], &t) != LEAF3)
        return 2;
    if (diff(&t.pair.y, &t) != PAIR_Y)
        return 3;
    if (diff(&t.b, &t) != ANON_B)
        return 4;
    if (diff(&t.u, &t) != table[0] || diff(&t.u.l, &t) != table[1])
        return 5;
    if (diff(&t.leaves[0].code[0], &t.leaves[0]) != table[2])
        return 6;
    if (diff(&t.leaves[1].weight, &t) != table[3])
        return 7;
    if (sizeof bound != diff(&t.pair, &t) + 1)
        return 8;
    if (diff(&m.buf[7], &m) != __builtin_offsetof(union Mixed, buf[7]))
        return 9;
    if (__builtin_offsetof(struct Tree, leaves[i].code[1]) != diff(&t.leaves[4].code[1], &t))
        return 10;
    switch (diff(&t.leaves[1], &t)) {
    case __builtin_offsetof(struct Tree, leaves[1]):
        break;
    default:
        return 11;
    }
    return (int)(code_off + LEAF3 + PAIR_Y + ANON_B) & 0x3f;
}
