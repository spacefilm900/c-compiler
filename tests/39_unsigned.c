int main(void) {
    unsigned u = 0u;
    unsigned char c = 0;
    unsigned long big;
    unsigned top;
    unsigned prod;
    int s;

    u -= 1u;
    if (u != 4294967295u)
        return 1;
    if (u / 2u != 2147483647u)
        return 2;
    if (u % 2u != 1u)
        return 3;
    if ((u >> 1) != 2147483647u)
        return 4;
    if ((1u << 31) != 2147483648u)
        return 5;
    c -= 1;
    if (c != 255)
        return 6;
    /* unsigned char promotes to int, so this division is signed. */
    if (c / 2 != 127)
        return 7;
    big = 1ul << 40;
    if ((unsigned)big != 0u)
        return 8;
    if ((unsigned)(big >> 32) != 256u)
        return 9;
    top = 0x80000000u >> 4;
    if (top != 0x08000000u)
        return 10;
    s = -8 >> 1;
    if (s != -4)
        return 11;
    if (!(2u > 1u))
        return 12;
    if (1u > 2u)
        return 13;
    prod = 65536u * 65536u;
    if (prod != 0u)
        return 14;
    u = 1u;
    u <<= 31;
    if (u != 2147483648u)
        return 15;
    if ((0x80000000u << 1) != 0u)
        return 16;
    return 0;
}
