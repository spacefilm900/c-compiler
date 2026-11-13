/* A struct whose size is not a multiple of eight returns in two
   registers. The second piece holds only the bytes the object has
   (SysV AMD64 psABI 3.2.3), so a copy of the result into a local must
   not write past the end of that local. Minimised from Csmith seeds
   121, 755, 819, 1468, 1841 and 1855 at -O0, where the overrun cleared
   or set a neighbouring int flag. */

int printf(const char *, ...);

struct Bits { unsigned f0 : 27; unsigned f1 : 6; unsigned f2 : 17; signed f3 : 24; };
struct Twelve { int a; int b; short c; char d; };
struct Eleven { char a[8]; char b, c, d; };
struct Ten { short a[4]; char b; char c; };

static struct Bits gb = {7765, 3, 141, 2755};
static struct Twelve gt = {1, 2, 3, 4};
static struct Eleven ge = {{1, 2, 3, 4, 5, 6, 7, 8}, 9, 10, 11};
static struct Ten gn = {{1, 2, 3, 4}, 5, 6};

static struct Bits get_bits(void) { return gb; }
static struct Twelve get_twelve(void) { return gt; }
static struct Eleven get_eleven(void) { return ge; }
static struct Ten get_ten(void) { return gn; }

static int take_bits(struct Bits b) { return b.f3 + b.f2; }
static int take_twelve(struct Twelve t) { return t.a + t.b + t.c + t.d; }

static void show(long val, const char *name, int flag)
{
    if (flag)
        printf("flag set for %s (%ld)\n", name, val);
}

int main(void)
{
    int flag = 0;
    get_bits();
    show(1, "bits", flag);
    get_twelve();
    show(2, "twelve", flag);
    get_eleven();
    show(3, "eleven", flag);
    get_ten();
    show(4, "ten", flag);

    char guard_e[4] = {0x55, 0x55, 0x55, 0x55};
    struct Eleven e = get_eleven();
    short guard_n = 0x1234;
    struct Ten n = get_ten();
    int guard_b = -1;
    struct Bits b = get_bits();
    printf("%d %d %d %d\n", e.d, n.c, b.f3, b.f2);
    printf("%d %d %d\n", guard_e[0] + guard_e[3], guard_n, guard_b);
    printf("%d %d\n", take_bits(gb), take_twelve(gt));
    return flag;
}
