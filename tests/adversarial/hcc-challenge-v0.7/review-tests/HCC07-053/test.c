/* HCC07-053: Omitted conditional evaluates first operand once
 * Basis: N1570 6.5.15; GCC manual Conditionals
 * GNU x ?: y skips second operand when first is nonzero and evaluates first exactly once.
 * Expected: exit 0; stdout: ok; mode: gnu11.
 */
#include <stdio.h>
#define CHECK(x) do { if (!(x)) return 1; } while (0)


int main(void) {
int i=0,j=0; int a=(++i ?: ++j); CHECK(a==1 && i==1 && j==0); i=-1; int b=(++i ?: ++j); CHECK(b==1 && i==0 && j==1);
    puts("ok"); return 0;
}
