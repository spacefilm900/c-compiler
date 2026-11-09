int main(void) {
    char c;
    unsigned char uc;
    short s;
    int i;
    unsigned u;
    unsigned long ul;
    char d;

    c = 255;
    if (c != -1)
        return 1;
    c = 128;
    if (c != -128)
        return 2;
    c = 1000;
    if (c != -24)
        return 3;
    uc = -1;
    if (uc != 255)
        return 4;
    s = 65535;
    if (s != -1)
        return 5;
    i = 2147483648L;
    if (i != -2147483648)
        return 6;
    u = -1;
    if (u != 4294967295u)
        return 7;
    ul = -1;
    if (ul + 1ul != 0ul)
        return 8;
    c = 100;
    c <<= 1;
    if (c != -56)
        return 9;
    uc = 255;
    uc += 1;
    if (uc != 0)
        return 10;
    d = 127;
    d++;
    if (d != -128)
        return 11;
    d = 0;
    --d;
    if (d != -1)
        return 12;
    return 0;
}
