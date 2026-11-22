/* HCC07-028: Named goto leaves only inner VLA declaration
 * Basis: 6.2.4p7; 6.8.6.1
 * Outer VLA remains live when a backward jump leaves an inner VLA scope.
 * Expected: exit 0; stdout: ok; mode: c11.
 */
#include <stdio.h>
#define CHECK(x) do { if (!(x)) return 1; } while (0)


int main(void) {
int n=1024; volatile int outer[n]; int i=0; outer[n-1]=17;
    loop: ;
    { volatile int inner[n]; inner[n-1]=i; i=inner[n-1]+1; if (i<12000) goto loop; }
    CHECK(outer[n-1]==17 && i==12000);
    puts("ok"); return 0;
}
