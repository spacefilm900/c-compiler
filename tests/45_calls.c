char idc(char);
unsigned char idu(unsigned char);
short ids(short);
unsigned short idus(unsigned short);
long idl(long);
unsigned long idul(unsigned long);

char idc(char c) {
    return c;
}

unsigned char idu(unsigned char u) {
    return u;
}

short ids(short s) {
    return s;
}

unsigned short idus(unsigned short s) {
    return s;
}

long idl(long x) {
    return x;
}

unsigned long idul(unsigned long x) {
    return x;
}

int widen(char c, unsigned char u, short s, unsigned short us) {
    return c + u + s + us;
}

char trunc_arg(int x) {
    return x;
}

int take_long(int a, long b) {
    return a + (int)b;
}

int main(void) {
    if (idc(-5) != -5)
        return 1;
    if (idu(200) != 200)
        return 2;
    if (ids(-300) != -300)
        return 3;
    if (idus(40000) != 40000)
        return 4;
    if (idl(-1L) != -1L)
        return 5;
    if (idul(1ul << 40) != (1ul << 40))
        return 6;
    if (widen(-1, 200, -2, 40000) != 40197)
        return 7;
    if (trunc_arg(1000) != -24)
        return 8;
    if (idc(1000) != -24)
        return 9;
    if (take_long(3, 4L) != 7)
        return 10;
    return 0;
}
