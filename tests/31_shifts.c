int shl = 1 << 4;
int shr = 64 >> 2;
int neg = -8 >> 1;
int sign = -1 >> 4;
int wide = (-2147483647 - 1) >> 1;
int add_first = 1 + 2 << 1; /* (1 + 2) << 1 = 6 */
int shift_second = 1 << 2 + 1; /* 1 << (2 + 1) = 8 */

int main(void) {
    int a = 1 << 3;  /* 8 */
    int b = 32 >> 3; /* 4 */
    int c = -16 >> 2; /* -4 */
    int d = a << b; /* 8 << 4 = 128 */
    if (shl != 16)
        return 1;
    if (shr != 16)
        return 2;
    if (neg != -4)
        return 3;
    if (sign != -1)
        return 4;
    if (wide != ((-2147483647 - 1) >> 1))
        return 5;
    if (add_first != 6)
        return 6;
    if (shift_second != 8)
        return 7;
    if (a != 8 || b != 4 || c != -4 || d != 128)
        return 8;
    if ((1 << 0) != 1)
        return 9;
    if ((8 >> 0) != 8)
        return 10;
    return 0;
}
