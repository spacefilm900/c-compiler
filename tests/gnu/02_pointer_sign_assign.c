/* GNU C: assigning between pointers whose pointees differ only in the
   signedness of an integer type, or that drop a qualifier of the pointee.
   C11 6.5.16.1p1 makes these constraint violations; GCC and Clang accept
   them in GNU mode with a warning (-Wpointer-sign, -Wdiscarded-qualifiers).
   Initialization, argument passing, and return use the same rule
   (C11 6.7.9p11, 6.5.2.2p7, 6.8.6.4p3). */

int printf(const char *, ...);

static unsigned int gu = 0xfffffffeu;
static int *gp = &gu;
static const char msg[] = "hi";

static int first(int *p) { return *p; }
static int *as_int(unsigned int *p) { return p; }

int main(void)
{
    int *a = 0;
    unsigned int *b = &gu;
    a = b;
    printf("%d\n", *a);
    signed char *sc;
    unsigned char bytes[2] = {200, 7};
    sc = bytes;
    printf("%d %d\n", sc[0], sc[1]);
    char *c = bytes;
    printf("%d\n", c[1]);
    const int *ci;
    ci = b;
    printf("%d\n", *ci);
    char *m = msg;
    printf("%s\n", m);
    long *l;
    unsigned long ul = 5;
    l = &ul;
    printf("%ld\n", *l);
    int *volatile vi = b;
    printf("%d %d %d\n", first(b), *as_int(b), *vi + *gp);
    return 0;
}
