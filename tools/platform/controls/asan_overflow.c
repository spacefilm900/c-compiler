/* Known-positive control for the AddressSanitizer run: a one-element
   overflow of a stack array. The sanitizer must report it. Never run in
   the gating suites. */
#include <stdio.h>

int main(void) {
    int a[4] = {0, 1, 2, 3};
    volatile int i = 4;
    a[i] = 5;
    printf("%d\n", a[0]);
    return 0;
}
