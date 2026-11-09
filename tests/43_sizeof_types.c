int main(void) {
    char c;
    signed char sc;
    unsigned char uc;
    short s;
    unsigned short us;
    int i;
    unsigned ui;
    long l;
    unsigned long ul;
    long long ll;
    unsigned long long ull;
    int long unsigned mixed;

    if (sizeof(c) != 1 || sizeof(char) != 1)
        return 1;
    if (sizeof(sc) != 1 || sizeof(signed char) != 1)
        return 2;
    if (sizeof(uc) != 1 || sizeof(unsigned char) != 1)
        return 3;
    if (sizeof(s) != 2 || sizeof(short) != 2 || sizeof(short int) != 2)
        return 4;
    if (sizeof(us) != 2 || sizeof(unsigned short) != 2 || sizeof(unsigned short int) != 2)
        return 5;
    if (sizeof(i) != 4 || sizeof(int) != 4 || sizeof(signed) != 4 || sizeof(signed int) != 4)
        return 6;
    if (sizeof(ui) != 4 || sizeof(unsigned) != 4 || sizeof(unsigned int) != 4)
        return 7;
    if (sizeof(l) != 8 || sizeof(long) != 8 || sizeof(long int) != 8 || sizeof(signed long) != 8)
        return 8;
    if (sizeof(ul) != 8 || sizeof(unsigned long) != 8 || sizeof(long unsigned int) != 8)
        return 9;
    if (sizeof(ll) != 8 || sizeof(long long) != 8 || sizeof(long long int) != 8)
        return 10;
    if (sizeof(ull) != 8 || sizeof(unsigned long long) != 8 || sizeof(unsigned long long int) != 8)
        return 11;
    if (sizeof(sizeof(int)) != 8 || sizeof(sizeof(c)) != 8)
        return 12;
    /* sizeof is unsigned long, so 4 - 5 wraps and is not < 10. */
    if ((sizeof(int) - 5) < 10)
        return 13;
    if (sizeof(mixed) != 8)
        return 14;
    return 0;
}
