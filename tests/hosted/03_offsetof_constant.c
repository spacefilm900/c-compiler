/* The <stddef.h> offsetof macro is an integer constant expression of
   type size_t (C11 7.19p3). It is used here in _Static_assert, an
   enumeration, file-scope and block-scope static initializers, array
   bounds, and a case label, and each value is checked against the
   address difference at run time. */

#include <stddef.h>
#include <stdio.h>

struct Point {
    short x;
    double y;
};

struct Shape {
    char kind;
    struct Point corners[4];
    struct {
        int r, g, b;
    } colour;
    int bits : 3;
    long trailer[];
};

_Static_assert(offsetof(struct Shape, kind) == 0, "kind first");
_Static_assert(offsetof(struct Shape, corners[3].y) == offsetof(struct Shape, corners)
                   + 3 * sizeof(struct Point) + offsetof(struct Point, y),
               "array element offset");
_Static_assert(_Generic(offsetof(struct Shape, colour), size_t: 1, default: 0),
               "offsetof has type size_t");

enum { GREEN = offsetof(struct Shape, colour.g), TRAILER = offsetof(struct Shape, trailer) };

static const size_t corner2 = offsetof(struct Shape, corners[2]);
static unsigned char scratch[offsetof(struct Shape, corners[1].y)];
static const size_t pieces[] = {offsetof(struct Point, x), offsetof(struct Point, y)};

int main(void) {
    static const size_t blue = offsetof(struct Shape, colour.b);
    int local[offsetof(struct Shape, colour) / sizeof(int)];
    struct Shape s;
    char *base = (char *)&s;

    if ((size_t)((char *)&s.corners[3].y - base) != offsetof(struct Shape, corners[3].y))
        return 1;
    if ((size_t)((char *)&s.corners[2] - base) != corner2)
        return 2;
    if ((size_t)((char *)&s.colour.g - base) != GREEN)
        return 3;
    if ((size_t)((char *)&s.colour.b - base) != blue)
        return 4;
    if ((size_t)((char *)s.trailer - base) != TRAILER)
        return 5;
    if (sizeof scratch != (size_t)((char *)&s.corners[1].y - base))
        return 6;
    if (sizeof local != offsetof(struct Shape, colour) / sizeof(int) * sizeof(int))
        return 7;
    if (pieces[0] != 0 || pieces[1] != (size_t)((char *)&s.corners[0].y - (char *)&s.corners[0]))
        return 8;
    switch ((size_t)((char *)&s.colour - base)) {
    case offsetof(struct Shape, colour):
        break;
    default:
        return 9;
    }
    printf("%zu %zu %zu %zu\n", corner2, (size_t)GREEN, blue, (size_t)TRAILER);
    return 0;
}
