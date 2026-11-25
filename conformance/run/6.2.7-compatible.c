/* N1570 6.2.7 compatible type and composite type */
#include "check.h"

extern int a[];
int a[4] = { 1, 2, 3, 4 };          /* composite type int[4] */
int f();                            /* no prototype */
int f(int x, double y);             /* composite gets the prototype */
int f(int x, double y) { return x + (int)y; }
typedef int I;
I g(I);
int g(int v) { return v * 2; }      /* typedefs are not new types */
enum col { RED, GREEN };
int (*fp)(int (*)[], int) ;
int h(int (*p)[3], int n) { return (*p)[n]; }

int main(void) {
    CHECK(sizeof a == 4 * sizeof(int));
    CHECK(f(1, 2.0) == 3);
    CHECK(g(4) == 8);
    fp = h;                          /* int (*)[] is compatible with int (*)[3] */
    int arr[3] = { 7, 8, 9 };
    CHECK(fp(&arr, 2) == 9);
    CHECK(_Generic((enum col)0, enum col: 1, default: 0));
    /* 6.2.7p1: qualified versions are not compatible with unqualified for _Generic purposes after lvalue conversion */
    const int ci = 1;
    CHECK(_Generic(ci, int: 1, default: 0));   /* lvalue conversion drops qualifiers */
    CHECK(_Generic(&ci, const int *: 1, int *: 2, default: 0) == 1);
    return DONE();
}
