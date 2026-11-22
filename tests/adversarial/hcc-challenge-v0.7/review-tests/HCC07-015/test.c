/* HCC07-015: Prior prototype uses promoted unsigned short and float
 * Basis: 6.7.6.3p15; 6.9.1; 6.5.2.2
 * Old-style definition compatible with promoted int and double; body sees narrow declared types on documented targets.
 * Expected: exit 0; stdout: ok; mode: c11.
 */
#include <stdio.h>
#define CHECK(x) do { if (!(x)) return 1; } while (0)

int f(int,double);
int f(a,b) unsigned short a; float b; { return a+(int)(2*b); }
int g(a,b) unsigned short a; float b; { return a+(int)(2*b); }
int main(void) {
CHECK(f(7,2.5)==12); CHECK(g(9,1.5f)==12);
    puts("ok"); return 0;
}
