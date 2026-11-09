void write(int **pp, int v) { **pp = v; }

int main(void) {
    int x = 1;
    int y = 2;
    int *p = &x;
    int **pp = &p;
    if (**pp != 1) return 1;
    *pp = &y;
    if (**pp != 2) return 2;
    write(pp, 8);
    if (y != 8) return 3;
    write(&p, 9);
    if (y != 9) return 4;
    p = &x;
    **pp = 6;
    if (x != 6) return 5;
    return 0;
}
