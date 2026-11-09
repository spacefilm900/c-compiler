int *choose(int c, int *a, int *b) { return c ? a : b; }
int *or_null(int c, int *a) { return c ? a : 0; }
int *or_voidnull(int c, int *a) { return c ? (void *)0 : a; }
void *mix(int c, int *a, void *b) { return c ? a : b; }

int main(void) {
    int x = 2;
    int y = 4;
    int *p = choose(1, &x, &y);
    int *q = choose(0, &x, &y);
    int *n = or_null(0, &x);
    int *m = or_voidnull(1, &x);
    void *v = mix(1, &x, 0);
    int *back = (int *)v;
    if (*p != 2 || *q != 4) return 1;
    if (n != 0 || m != 0) return 2;
    if (back != &x || *back != 2) return 3;
    if ((1 ? &x : &y) != &x) return 4;
    if ((0 ? &x : (void *)0) != 0) return 5;
    return 0;
}
