int swap(int *a, int *b) {
    int t = *a;
    *a = *b;
    *b = t;
    return 0;
}

int main(void) {
    int x = 3;
    int y = 9;
    int *px = &x;
    swap(px, &y);
    if (x != 9) return 1;
    if (y != 3) return 2;
    if (*px != 9) return 3;
    *&x = 4;
    if (x != 4) return 4;
    if (&*px != px) return 5;
    return 0;
}
