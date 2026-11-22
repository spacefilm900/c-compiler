/* HCC07-040: Atomic pointer increment and decrement preserve element stride
 * Basis: 6.5.2.4p2; 6.5.3.1; 6.5.6
 * Language-level increment and decrement on atomic pointer use ordinary pointer arithmetic.
 * Expected: exit 0; stdout: ok; mode: c11.
 */
#include <stdio.h>
#define CHECK(x) do { if (!(x)) return 1; } while (0)


int main(void) {
int a[5]={2,4,6,8,10}; _Atomic(int *) p=a; int *old=p++; int *mid=++p; int *before=p--; int *q=p; CHECK(old==a && mid==a+2 && before==a+2 && q==a+1 && *q==4);
    puts("ok"); return 0;
}
