/* <stdalign.h> (C11 7.15): alignas and alignof spell _Alignas and _Alignof. */

#include <stdalign.h>
#include <stdio.h>

alignas(16) int g;
struct S { char c; alignas(8) int n; };

int main(void) {
    alignas(32) unsigned char buf[4];
    if (__alignas_is_defined != 1 || __alignof_is_defined != 1)
        return 1;
    if (alignof(int) != _Alignof(int))
        return 2;
    if (alignof(struct S) != 8)
        return 3;
    if (((unsigned long)&g) % 16 != 0)
        return 4;
    if (((unsigned long)&buf) % 32 != 0)
        return 5;
    printf("%d %d %d\n", (int)alignof(int), (int)alignof(struct S),
           (int)sizeof(struct S));
    return 0;
}
