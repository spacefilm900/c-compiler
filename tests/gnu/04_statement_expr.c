/* GNU C: statement expressions (GCC manual, "Statements and Declarations
   in Expressions"). The value is that of the last expression statement;
   a block that ends in anything else has type void. */

int printf(const char *, ...);

struct pair { int a; long b; };

static int counter;
static int next(void) { return ++counter; }

int main(void)
{
    int a = ({ int t = 3; t * 2; });
    int b = ({ int s = 0; for (int i = 1; i <= 4; i++) s += i; s; });
    int c = ({ int _v = next(); _v * _v; });
    int d = ({ int x = 1; ({ int y = x + 1; y * 10; }) + x; });
    long e = ({ struct pair p = {2, 40}; p.a + p.b; });
    struct pair q = ({ struct pair r = {7, 8}; r.b += 1; r; });
    int n = ({ 5; }) + ({ int z = 2; z; });
    char *s = ({ static char text[] = "hi"; text; });
    double f = ({ float g = 1.5f; g; }) * 2;
    ({ counter += 10; });
    (void)({ int unused = 4; unused; });
    int g = 0;
    while (({ g++; g < 5; }))
        ;
    if (({ int w = a; w == 6; }))
        printf("six\n");
    int h = 1 ? ({ int u = 11; u; }) : ({ 12; });
    int k = 0 && ({ printf("not evaluated\n"); 1; });
    printf("%d %d %d %d %ld\n", a, b, c, d, e);
    printf("%d %ld %d %s %.1f %d %d %d %d\n", q.a, q.b, n, s, f, counter, g, h, k);
    for (int i = 0; i < 3; i++) {
        int v = ({ int r = i; if (r == 1) { r = 100; } r; });
        printf("%d ", v);
    }
    printf("\n");
    return ({ struct pair z = {0, 0}; z.a; });
}
