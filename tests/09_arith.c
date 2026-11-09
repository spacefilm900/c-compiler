int main(void) {
    int a = 1 + 2 * 3 - 4; /* 3 */
    int b = 20 / 6;        /* 3 */
    int c = 20 % 6;        /* 2 */
    int d = -5 + 8;        /* 3 */
    int e = !0 + !1;       /* 1 */
    int f = 8 - 2 - 1;     /* 5 */
    int g = +4 - 4;        /* 0 */
    int n = -7 / 2;        /* -3 */
    int m = -7 % 2;        /* -1 */
    int t = (n + 3) * 10 + (m + 1); /* 0 when division truncates toward zero */
    return a + b + c + d + e + f + g + t; /* 17 */
}
