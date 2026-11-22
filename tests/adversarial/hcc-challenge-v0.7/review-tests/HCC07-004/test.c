/* HCC07-004: Extern alignment and one definition
 * Basis: 6.7.5p7; 6.2.2
 * An extern declaration without alignment may precede an aligned definition; subsequent equivalent alignment is allowed.
 * Expected: exit 0; stdout: ok; mode: c11.
 */
#include <stdio.h>
#define CHECK(x) do { if (!(x)) return 1; } while (0)
#include <stdint.h>
extern int g;
_Alignas(32) int g=42;
extern _Alignas(32) int g;
int main(void) {
CHECK((uintptr_t)&g%32==0 && g==42);
    puts("ok"); return 0;
}
