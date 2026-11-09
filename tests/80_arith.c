int printf(const char *, ...);

struct S { int a; int b; char c; };

int main(void) {
    struct S arr[4];
    int i = 0;
    while (i < 4) {
        arr[i].a = i;
        arr[i].b = i * 10;
        arr[i].c = i;
        i++;
    }
    struct S *p = arr;
    struct S *q = p + 3;
    printf("%ld\n", q - p);
    printf("%ld\n", (long)((char *)q - (char *)p));
    printf("%lu\n", sizeof(struct S));
    printf("%d %d\n", q->a, (p + 1)->b);
    p += 2;
    printf("%d\n", p->a);
    p--;
    printf("%d\n", p->b);
    struct S *end = &arr[4];
    printf("%ld\n", end - arr);
    return 0;
}
