/* HCC07-057: GNU overflow builtin single evaluation
 * Basis: N1570 6.5.2.2; GCC manual Integer Overflow Builtins
 * Result conversion checked at destination width, and each input and pointer evaluated once.
 * Expected: exit 0; stdout: ok; mode: gnu11.
 */
#include <stdio.h>
#define CHECK(x) do { if (!(x)) return 1; } while (0)


int main(void) {
int i=250,j=0; unsigned char a[2]={0,0}; int o=__builtin_add_overflow(i++,10,&a[j++]); CHECK(o && a[0]==4 && i==251 && j==1);
    puts("ok"); return 0;
}
