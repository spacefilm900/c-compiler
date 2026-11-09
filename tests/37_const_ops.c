int bits = (6 & 3) | (1 ^ 4);
int tilde = ~0;
int shifted = 1 << 4;
int arith = -8 >> 1;
int pick = 1 ? 7 : 1 / 0;
int other = 0 ? 1 / 0 : 5;
int sz = sizeof(int);
int quiet = 0 && (1 / 0);
int loud = 1 || (1 << 40);
int n;
int skipped = 0 ? n++ : 9;
int foo(void);
int uneval = 1 ? 4 : foo();

int main(void) {
    if (bits != 7)
        return 1;
    if (tilde != -1)
        return 2;
    if (shifted != 16)
        return 3;
    if (arith != -4)
        return 4;
    if (pick != 7 || other != 5)
        return 5;
    if (sz != 4)
        return 6;
    if (quiet != 0 || loud != 1)
        return 7;
    if (n != 0 || skipped != 9)
        return 8;
    if (uneval != 4)
        return 9;
    return 0;
}
