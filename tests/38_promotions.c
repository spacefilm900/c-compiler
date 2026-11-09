int main(void) {
    char c = -1;
    unsigned char uc = 200;
    short s = -2;
    unsigned short us = 40000;
    int i = -1;
    unsigned u = 1u;
    long l = -1L;
    unsigned long ul = 1ul;
    int sum;

    if (c + 1 != 0)
        return 1;
    if (uc + 1 != 201)
        return 2;
    if (s + 1 != -1)
        return 3;
    if (us + 1 != 40001)
        return 4;
    /* -1 converted to unsigned int is not less than 1. */
    if (i < u)
        return 5;
    /* long can represent every unsigned int, so this stays signed. */
    if (!(l < u))
        return 6;
    /* unsigned long has greater rank, so -1 becomes a huge unsigned value. */
    if (l < ul)
        return 7;
    if ((-1LL + 0UL) < 1)
        return 8;
    if (!((-1L + 0U) < 0))
        return 9;
    if (us < -1)
        return 10;
    if (!(c < 0))
        return 11;
    if (uc < 0)
        return 12;
    sum = uc + 1;
    if (sum != 201)
        return 13;
    uc = 255;
    if (uc + 1 != 256)
        return 14;
    if ((1 ? -1 : 1u) != 4294967295u)
        return 15;
    if ((long)(1 ? -1 : 1u) != 4294967295L)
        return 16;
    return 0;
}
