/* Static initializers built from addresses:
   - `&1[a]` is `&a[1]`; either subscript operand may be the pointer
     (C11 6.5.2.1p2).
   - A pointer converted to `_Bool` is 1 unless it is null (C11 6.3.1.2),
     and the address of an object or function is never null (6.3.2.3p3). */
int printf(const char *, ...);

int a[4] = {10, 20, 30, 40};
struct S { int v[3]; } s = {{1, 2, 3}};
int *p = &1[a];
int *q = &2[s.v];
int *r = 3 + a;
char *t = &1["xyz"];

int g;
int arr[3];
void fn(void) {}
_Bool b1 = &g;
_Bool b2 = (void *)0;
_Bool b3 = arr;
_Bool b4 = fn;
_Bool b5 = "x";
_Bool b6 = &arr[2];
_Bool b7 = (int *)0;
struct P { _Bool ok; int *p; } ps[2] = { { &g, &g }, { 0, 0 } };

int main(void) {
    static _Bool local = &g;
    printf("%d %d %d %c\n", *p, *q, *r, *t);
    printf("%d %d %d %d %d %d %d | %d %d %d\n", b1, b2, b3, b4, b5, b6, b7,
           ps[0].ok, ps[1].ok, local);
    return *p == 20 && *q == 3 && b1 && !b2 ? 0 : 1;
}
