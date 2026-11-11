int printf(const char *, ...);
unsigned long strlen(const char *);
struct P { int x, y; };
union U { int i; float f; char c[8]; };
struct N { struct P p; int a[3]; };
static int *gp = (int[]){10, 20, 30};
static struct P *gs = &(struct P){.y = 5, .x = 4};
static const char *gstr = (const char[]){"hi"};
int *gq = (int[3]){[2] = 7};
static int gsum(const int *p, int n) { int s = 0; while (n--) s += *p++; return s; }
static struct P mk(int a) { return (struct P){a, a * 2}; }
static int area(struct P p) { return p.x * p.y; }
int main(void) {
    int *p = (int[]){1, 2, 3, 4};
    printf("%d %d %d %d\n", p[0], p[3], gp[1], gq[2]);
    printf("%d %d %s\n", gs->x, gs->y, gstr);
    struct P q = (struct P){.y = 9};
    printf("%d %d\n", q.x, q.y);
    printf("%d\n", area((struct P){3, 4}));
    printf("%d\n", ((struct P){7, 8}).y);
    printf("%d\n", (int){42});
    int *r = &(int){5};
    *r += 1;
    printf("%d\n", *r);
    (struct P){1, 2}.x = 3; /* lvalue */
    union U u = (union U){.f = 1.5f};
    printf("%f\n", u.f);
    printf("%d\n", ((union U){.c = {1, 2}}).c[1]);
    struct N n = (struct N){.a = {1, 2, 3}, .p = {.x = 9}};
    printf("%d %d %d\n", n.p.x, n.a[2], n.p.y);
    printf("%zu %zu\n", sizeof (int[]){1, 2, 3}, sizeof((struct N){0}));
    printf("%d\n", gsum((const int[]){1, 2, 3, 4, 5}, 5));
    int total = 0;
    for (int i = 0; i < 3; i++) {
        int *c = (int[2]){i, i * 10};
        total += c[0] + c[1];
        c[0] = 100;
    }
    printf("%d %d\n", total, mk(3).y);
    char *s = (char[]){"abc"};
    s[0] = 'X';
    printf("%s %zu\n", s, strlen((char[]){'q', 'r', 0}));
    struct P *arr = (struct P[]){{1, 2}, {3, 4}};
    printf("%d\n", arr[1].x + arr[0].y);
    double d = (double){1} / 4;
    printf("%f\n", d);
    return 0;
}
