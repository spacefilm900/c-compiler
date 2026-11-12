/* __builtin_offsetof with a subscript that is not an integer constant
   expression (a GNU extension) is evaluated at run time. The result
   must equal the address difference measured on a real object, at
   every optimization level, for signed, unsigned, narrow, and wide
   index types, nested designators, and a mix of constant and
   non-constant subscripts. No headers: the builtin is used directly. */

typedef unsigned long size_t;

struct Inner {
    char tag;
    short s[3];
    double d[2];
};

struct Outer {
    char c;
    int a[4];
    struct Inner in[5];
    long long tail[3][7];
};

static size_t diff(const void *member, const void *base) {
    return (size_t)((const char *)member - (const char *)base);
}

static int volatile_index(int value) {
    volatile int v = value;
    return v;
}

int main(void) {
    struct Outer o;
    int failures = 0;
    for (int i = 0; i < 4; i++) {
        int k = volatile_index(i);
        unsigned u = (unsigned)k;
        unsigned char uc = (unsigned char)k;
        long long ll = k;
        if (__builtin_offsetof(struct Outer, a[k]) != diff(&o.a[k], &o))
            failures |= 1;
        if (__builtin_offsetof(struct Outer, in[u].s[2]) != diff(&o.in[u].s[2], &o))
            failures |= 2;
        if (__builtin_offsetof(struct Outer, in[3].d[uc % 2]) != diff(&o.in[3].d[uc % 2], &o))
            failures |= 4;
        if (__builtin_offsetof(struct Outer, tail[ll % 3][k + 2]) != diff(&o.tail[ll % 3][k + 2], &o))
            failures |= 8;
        if (__builtin_offsetof(struct Outer, in[k].tag) != diff(&o.in[k].tag, &o))
            failures |= 16;
    }
    if (sizeof(__builtin_offsetof(struct Outer, a[volatile_index(1)])) != sizeof(size_t))
        failures |= 32;
    return failures;
}
