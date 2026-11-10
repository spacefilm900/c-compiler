int main(void) {
    volatile int x = 1;
    volatile int *p = &x;
    x = 2;
    if (*p != 2) {
        return 1;
    }
    *p = 3;
    if (x != 3) {
        return 2;
    }
    volatile struct {
        int a;
        int b;
    } s;
    s.a = 4;
    s.b = 5;
    if (s.a + s.b != 9) {
        return 3;
    }
    return 0;
}
