/* Regression control (caller, built by Clang): must print 42. */
#include <stdio.h>
struct s { long a; void *b; };
long get_a(struct s x);
void sink(long *p) { *p = 0; }

int main(void) {
    struct s v = {42, 0};
    long r = get_a(v);
    printf("%ld\n", r);
    return r != 42;
}
