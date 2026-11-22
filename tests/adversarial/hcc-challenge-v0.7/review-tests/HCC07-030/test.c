/* HCC07-030: Prototype star replaced by runtime bound in definition
 * Basis: 6.7.6.2p4,p5; 6.7.6.3p7
 * Prototype-scope star is compatible with later parameter whose inner dimension is runtime-valued.
 * Expected: exit 0; stdout: ok; mode: c11.
 */
#include <stdio.h>
#define CHECK(x) do { if (!(x)) return 1; } while (0)

int f(int,int,int [*][*]);
int f(int m,int n,int a[m][n]) { return a[m-1][n-1]; }
int main(void) {
int a[2][3]={{1,2,3},{4,5,6}}; CHECK(f(2,3,a)==6);
    puts("ok"); return 0;
}
