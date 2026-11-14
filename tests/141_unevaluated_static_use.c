/* A static function that is never defined may appear in an operand that is
   not evaluated: a sizeof operand (C11 6.9p3), a _Generic controlling
   expression, or an unselected _Generic association (6.5.1.1p3). No call
   is emitted, so no definition is needed. String literals there make no
   object either. */

int printf(const char *, ...);
static int missing(void);
static double also_missing(int);
int main(void) {
    int a = (int)sizeof(missing());
    int b = _Generic(missing(), int: 1, default: 2);
    int c = _Generic(0, int: 3, default: missing());
    int d = _Generic(1.0f, float: 4, double: (int)also_missing(1), default: (int)"no"[0]);
    int e = (int)sizeof(also_missing(2) + "abc"[0]);
    printf("%d %d %d %d %d\n", a, b, c, d, e);
    return 0;
}
