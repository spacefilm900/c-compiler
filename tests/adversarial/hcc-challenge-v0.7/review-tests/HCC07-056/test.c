/* HCC07-056: Zero-length tail member and padding
 * Basis: N1570 6.7.6.2; GCC manual Zero Length
 * GNU zero length member at end contributes alignment but zero element storage; no access beyond it.
 * Expected: exit 0; stdout: ok; mode: gnu11.
 */
#include <stdio.h>
#define CHECK(x) do { if (!(x)) return 1; } while (0)
#include <stddef.h>
struct S { char c; int tail[0]; };
int main(void) {
CHECK(sizeof(((struct S*)0)->tail)==0 && offsetof(struct S,tail)%_Alignof(int)==0 && sizeof(struct S)>=offsetof(struct S,tail));
    puts("ok"); return 0;
}
