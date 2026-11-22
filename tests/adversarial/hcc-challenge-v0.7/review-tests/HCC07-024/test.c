/* HCC07-024: VLA typedef captured across changed bounds
 * Basis: 6.7.6.2p5; 6.5.3.4p2
 * Each VLA instance keeps its captured bound; typedef and nested declarator capture separately.
 * Expected: exit 0; stdout: ok; mode: c11.
 */
#include <stdio.h>
#define CHECK(x) do { if (!(x)) return 1; } while (0)


int main(void) {
int n=3; typedef int Row[n]; n=5; Row table[n]; n=7;
    table[4][2]=42; CHECK(sizeof(Row)==3*sizeof(int)); CHECK(sizeof table==15*sizeof(int)); CHECK(table[4][2]==42);
    puts("ok"); return 0;
}
