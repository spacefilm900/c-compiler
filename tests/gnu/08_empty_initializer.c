/* GNU C: an empty initializer list `{}` initializes the object as if it
   had static storage duration (zero). C11 6.7.9 requires at least one
   initializer. */

int printf(const char *, ...);

struct s { int a; double b; char c[3]; };
static struct s gs = {};
int garr[4] = {};

int main(void)
{
    struct s ls = {};
    int arr[3] = {};
    int scalar = {};
    struct s nested[2] = { {}, { 1, 2.0, {} } };
    printf("%d %.1f %d\n", gs.a, gs.b, gs.c[2]);
    printf("%d %d %d\n", garr[3], ls.a, ls.c[0]);
    printf("%d %d %d %.1f\n", arr[2], scalar, nested[0].a, nested[1].b);
    return nested[1].c[1];
}
