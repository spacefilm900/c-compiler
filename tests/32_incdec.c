int g = 1;

int post_param(int n) {
    return n++;
}

int pre_param(int n) {
    return ++n;
}

int inc_global(void) {
    return ++g;
}

int dec_global(void) {
    return g--;
}

int read_global(void) {
    return g;
}

int main(void) {
    int x = 5;
    int y = x++;
    int z = ++x;
    int local;
    int i;
    int sum;
    if (y != 5)
        return 1;
    if (z != 7 || x != 7)
        return 2;
    if (post_param(4) != 4)
        return 3;
    if (pre_param(4) != 5)
        return 4;
    if (inc_global() != 2)
        return 5;
    if (dec_global() != 2)
        return 6;
    if (read_global() != 1)
        return 7;
    local = 4;
    local++;
    if (read_global() != 1)
        return 8;
    if (local != 5)
        return 9;
    x = 0;
    if (x++)
        return 10;
    if (x != 1)
        return 11;
    if (!++x)
        return 12;
    if (x != 2)
        return 13;
    sum = 0;
    for (i = 0; i < 4; ++i)
        sum = sum + i;
    if (sum != 6)
        return 14;
    i = 3;
    if (i-- != 3 || i != 2)
        return 15;
    if (--i != 1)
        return 16;
    return 0;
}
