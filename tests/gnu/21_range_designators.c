/* GNU C: range designators `[first ... last] = value` (GCC manual,
   "Designated Inits"). QuickJS fills the rest of its dispatch table with
   `[ OP_COUNT ... 255 ] = &&case_default`. */
int printf(const char *, ...);
struct P { int x, y; };
enum { N = 6 };
static int a[10] = { [2 ... 4] = 7, 9, [8 ... 8] = 1 };
static unsigned char b[N + 2] = { [0 ... N + 1] = 'z', [3] = 'q' };
static struct P ps[5] = { [1 ... 3].y = 5, [1 ... 2] = { 8 } };
static int m[3][4] = { [0 ... 2][1 ... 2] = 4 };
static const char *names[4] = { [0 ... 3] = "x", [2] = "two" };
static int t[] = { [0 ... 5] = 3 };
int main(void) {
    int k = 11;
    int la[6] = { [1 ... 4] = k * 2, 1 };
    for (int i = 0; i < 10; i++) printf("%d ", a[i]);
    for (int i = 0; i < N + 2; i++) printf("%c", b[i]);
    for (int i = 0; i < 5; i++) printf(" %d,%d", ps[i].x, ps[i].y);
    for (int i = 0; i < 3; i++) for (int j = 0; j < 4; j++) printf(" %d", m[i][j]);
    for (int i = 0; i < 4; i++) printf(" %s", names[i]);
    for (int i = 0; i < 6; i++) printf(" %d", la[i]);
    printf(" %zu\n", sizeof t / sizeof t[0]);
    return 0;
}
