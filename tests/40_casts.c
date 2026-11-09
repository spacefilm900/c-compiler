int main(void) {
    char c;
    unsigned char uc;
    short s;
    unsigned u;
    int x;
    unsigned long ul;
    long l;
    int y;

    c = (char)1000;
    if (c != -24)
        return 1;
    uc = (unsigned char)1000;
    if (uc != 232)
        return 2;
    s = (short)65535;
    if (s != -1)
        return 3;
    u = (unsigned)-1;
    if (u != 4294967295u)
        return 4;
    x = (int)2147483648L;
    if (x != -2147483648)
        return 5;
    ul = (unsigned long)-1;
    if (ul + 1ul != 0ul)
        return 6;
    l = (long)(unsigned)-1;
    if (l != 4294967295L)
        return 7;
    y = 1;
    (void)y;
    (void)(y + 1);
    if (y != 1)
        return 8;
    if ((int)(char)-1 != -1)
        return 9;
    if ((int)(unsigned char)255 != 255)
        return 10;
    if ((unsigned char)(char)200 != 200)
        return 11;
    if ((char)(unsigned char)200 != -56)
        return 12;
    return 0;
}
