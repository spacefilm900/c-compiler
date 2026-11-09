int printf(const char *, ...);

struct Inner { int x; char y; };
struct Outer { char c; struct Inner in; int z; };
struct Pair { int x; int y; };

struct Pair gp = {1, 2};
struct Outer go = {3, {4, 5}, 6};
struct Outer partial = {7};
static struct Pair hidden = {8, 9};
struct Pair table[3] = {{1, 2}, {3}};
int g = 10;
struct Ref { int *p; struct Pair pair; };
struct Ref ref = {&g, {11, 12}};
struct Pair *at = &table[1];

int main(void) {
    printf("%d %d\n", gp.x, gp.y);
    printf("%d %d %d %d\n", go.c, go.in.x, go.in.y, go.z);
    printf("%d %d %d\n", partial.c, partial.in.x, partial.z);
    printf("%d %d\n", hidden.x, hidden.y);
    printf("%d %d %d %d\n", table[0].y, table[1].x, table[2].x, table[2].y);
    *ref.p = 13;
    printf("%d %d %d\n", g, ref.pair.x, ref.pair.y);
    printf("%d %d\n", at->x, at->y);
    gp.x = 20;
    printf("%d\n", gp.x);
    return 0;
}
