int main(void) {
    int a = 6 & 3;          /* 2 */
    int b = 6 | 1;          /* 7 */
    int c = 6 ^ 3;          /* 5 */
    int d = ~0;             /* -1 */
    int e = (~1) & 15;      /* 14 */
    int f = 7 | 1 ^ 2;      /* 7 | (1 ^ 2) = 7 */
    int g = 5 & 3 == 1;     /* 5 & (3 == 1) = 0 */
    int h = 2 + (1 | 0 && 0); /* 2 + ((1 | 0) && 0) = 2 */
    int i = ~(~5);          /* 5 */
    if (a != 2)
        return 1;
    if (b != 7)
        return 2;
    if (c != 5)
        return 3;
    if (d != -1)
        return 4;
    if (e != 14)
        return 5;
    if (f != 7)
        return 6;
    if (g != 0)
        return 7;
    if (h != 2)
        return 8;
    if (i != 5)
        return 9;
    return 0;
}
