int main(void) {
    int x = 5;
    int *p = &x;
    long lp = (long)p;
    unsigned long ulp = (unsigned long)p;
    int *back = (int *)lp;
    int *back2 = (int *)ulp;
    void *vp = (void *)p;
    int *from_void = vp;
    char *cp = (char *)p;
    int *from_char = (int *)cp;
    int *all = (int *)(int)-1;
    int *all_long = (int *)(long)-1;
    int *low = (int *)(unsigned)-1;
    int *low_long = (int *)(unsigned long)(unsigned)-1;
    int *zero = (int *)0;
    if (back != p || back2 != p) return 1;
    if ((unsigned long)lp != ulp) return 2;
    if (from_void != p) return 3;
    if (from_char != p) return 4;
    if (*from_void != 5) return 5;
    if (all != all_long) return 6;
    if (low != low_long) return 7;
    if (all == low) return 8;
    if (zero != 0) return 9;
    if ((int *)(long)0 != 0) return 10;
    return 0;
}
