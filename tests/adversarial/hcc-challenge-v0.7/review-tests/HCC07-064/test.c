/* HCC07-064: GNU constructor runs before main
 * Basis: N1570 5.1.2.2.1; GCC manual Common Function Attributes
 * A single constructor runs before main under documented GNU attribute contract; no ordering assumption among constructors.
 * Expected: exit 0; stdout: ok; mode: gnu11.
 */
#include <stdio.h>
#define CHECK(x) do { if (!(x)) return 1; } while (0)

static int value;
static void init(void) __attribute__((constructor));
static void init(void) {value=37;}
int main(void) {
CHECK(value==37);
    puts("ok"); return 0;
}
