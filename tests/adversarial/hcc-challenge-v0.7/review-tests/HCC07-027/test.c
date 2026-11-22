/* HCC07-027: Nested break and continue release scopes
 * Basis: 6.2.4p7; 6.8.6.2; 6.8.6.3
 * Storage follows VLA scope lifetime across repeated exits; large cumulative allocation catches missing stack restoration.
 * Expected: exit 0; stdout: ok; mode: c11.
 */
#include <stdio.h>
#define CHECK(x) do { if (!(x)) return 1; } while (0)


int main(void) {
int total=0;
    for (int i=0;i<20000;i++) { int n=512+(i%3); volatile int a[n]; a[n-1]=i%5;
        if (i%2==0) continue;
        for (int j=0;j<3;j++) { volatile int b[n]; b[0]=2; total+=b[0]; break; }
        total+=a[n-1]; }
    CHECK(total==40000);
    puts("ok"); return 0;
}
