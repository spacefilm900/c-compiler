char c = 1000;
unsigned char uc = -1;
unsigned u = 1u << 31;
unsigned wrap = 4294967295u + 1u;
long sh = 1L << 40;
int from_hex = 0x80000000;
int from_long = 2147483648;
unsigned long neg = -1;
int sz = sizeof(sizeof(int));
int shifted = (int)(1L << 40);
unsigned char low = (unsigned char)(1ul << 40);
unsigned long cond_ul = 1 ? -1 : 1u;
int cond_cmp = (1 ? -1 : 1u) > 0;

int main(void) {
    if (c != -24)
        return 1;
    if (uc != 255)
        return 2;
    if (u != 2147483648u)
        return 3;
    if (wrap != 0u)
        return 4;
    if ((sh >> 40) != 1L)
        return 5;
    if (from_hex != -2147483648)
        return 6;
    if (from_long != -2147483648)
        return 7;
    if (neg + 1ul != 0ul)
        return 8;
    if (sz != 8)
        return 9;
    if (shifted != 0)
        return 10;
    if (low != 0)
        return 11;
    if (cond_ul != 4294967295ul)
        return 12;
    if (cond_cmp != 1)
        return 13;
    return 0;
}
