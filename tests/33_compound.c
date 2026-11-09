int g = 20;
int extra;

int add_extra(void) {
    extra = extra + 1;
    return 3;
}

int scale(int n) {
    n *= 3 + 1;
    n += 1;
    return n;
}

int main(void) {
    int x;
    g += 4;
    g -= 10;
    g *= 2;
    g /= 4;
    g %= 5;
    g &= 7;
    g |= 8;
    g ^= 3;
    g <<= 2;
    g >>= 1;
    if (g != 18)
        return 1;
    extra = 0;
    x = 10;
    x += add_extra();
    if (x != 13 || extra != 1)
        return 2;
    x = 10;
    x += (extra = extra + 1, 4);
    if (x != 14 || extra != 2)
        return 3;
    x = 2;
    x += x += 1;
    if (x != 6)
        return 4;
    if (scale(3) != 13)
        return 5;
    x = 8;
    x /= 3;
    x %= 3;
    if (x != 2)
        return 6;
    x = 1;
    x <<= 3;
    x >>= 1;
    if (x != 4)
        return 7;
    return 0;
}
