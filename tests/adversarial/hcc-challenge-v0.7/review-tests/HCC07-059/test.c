/* HCC07-059: Choose expression unselected side effect
 * Basis: N1570 6.5.1.1 analogous; GCC manual Other Builtins
 * Chosen expression is evaluated, unchosen expression is not.
 * Expected: exit 0; stdout: ok; mode: gnu11.
 */
#include <stdio.h>
#define CHECK(x) do { if (!(x)) return 1; } while (0)


int main(void) {
int i=1,j=3; int x=__builtin_choose_expr(0,++i,++j); CHECK(i==1 && j==4 && x==4); CHECK(__builtin_types_compatible_p(int, const int));
    puts("ok"); return 0;
}
