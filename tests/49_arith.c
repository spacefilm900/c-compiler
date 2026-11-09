long dchar(char *a, char *b) { return b - a; }
long dshort(short *a, short *b) { return b - a; }
long dint(int *a, int *b) { return b - a; }
long dlong(long *a, long *b) { return b - a; }

int main(void) {
    char cs[6];
    short ss[4];
    int is[5];
    long ls[3];
    int i;
    for (i = 0; i < 6; i = i + 1) cs[i] = (char)(i + 1);
    for (i = 0; i < 4; i = i + 1) ss[i] = (short)(i + 1);
    for (i = 0; i < 5; i = i + 1) is[i] = i + 1;
    for (i = 0; i < 3; i = i + 1) ls[i] = i + 1;
    if (dchar(&cs[0], &cs[5]) != 5) return 1;
    if (dshort(&ss[0], &ss[3]) != 3) return 2;
    if (dint(&is[1], &is[4]) != 3) return 3;
    if (dlong(&ls[0], &ls[2]) != 2) return 4;
    int *p = is + 1;
    if (*p != 2) return 5;
    p += 2;
    if (*p != 4) return 6;
    p -= 1;
    if (*p != 3) return 7;
    if (*(p + 1) != 4) return 8;
    if (*(1 + p) != 4) return 9;
    ++p;
    if (*p != 4) return 10;
    p++;
    if (p != is + 4) return 11;
    --p;
    if (*p != 4) return 12;
    p--;
    if (*p != 3) return 13;
    char *c = cs;
    c += 2;
    if (*c != 3) return 14;
    short *s = ss + 2;
    if (s - ss != 2) return 15;
    if ((is + 4) - (is + 1) != 3) return 16;
    return 0;
}
