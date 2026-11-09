int a = 1 + 2 * 3;
int b = -5 / 2;
int c = -5 % 2;
int d = !0 + (1 < 2);
int e = 0 || 3 && 4;
int f = 8 - 3 - 1;
int g = (2 + 3) * (8 / 4);
int h = 2147483647 + 1;
int z;
int w = 0 && (1 / 0);
int v = 1 || (1 / 0);
int u = 0 && z;

int main(void) {
    if (a != 7)
        return 1;
    if (b != -2)
        return 2;
    if (c != -1)
        return 3;
    if (d != 2)
        return 4;
    if (e != 1)
        return 5;
    if (f != 4)
        return 6;
    if (g != 10)
        return 7;
    if (h != (-2147483647 - 1))
        return 8;
    if (z != 0)
        return 9;
    if (w != 0)
        return 10;
    if (v != 1)
        return 11;
    if (u != 0)
        return 12;
    return 0;
}
