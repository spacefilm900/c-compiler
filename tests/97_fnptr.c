int add(int a, int b) { return a + b; }

int apply(int (*fn)(int, int), int a, int b) { return fn(a, b); }

int main(void) {
    int (*p)(int, int) = add;
    int (*q)(int, int) = &add;
    if (p(2, 3) != 5) {
        return 1;
    }
    if ((*q)(4, 6) != 10) {
        return 2;
    }
    return apply(add, 20, 22);
}
