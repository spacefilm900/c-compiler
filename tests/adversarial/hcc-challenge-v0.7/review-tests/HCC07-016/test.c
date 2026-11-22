/* HCC07-016: Old-style definition declaration order differs
 * Basis: 6.9.1p6; 6.5.2.2
 * Identifier list, not declaration order, determines argument positions.
 * Expected: exit 0; stdout: ok; mode: c11.
 */
#include <stdio.h>
#define CHECK(x) do { if (!(x)) return 1; } while (0)

int f(a,b,c) short c; double b; signed char a; { return 100*a+10*(int)b+c; }
int main(void) {
CHECK(f(3,5.0,7)==357);
    puts("ok"); return 0;
}
