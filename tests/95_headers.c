#include <assert.h>
#include <ctype.h>
#include <limits.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

int main(void) {
    char *p;
    uint32_t n;
    bool flag;
    p = malloc(8);
    if (p == NULL) {
        return 1;
    }
    p[0] = 'a';
    p[1] = 'b';
    p[2] = 'c';
    p[3] = 0;
    if (strlen(p) != 3) {
        return 2;
    }
    if (!isdigit('4') || isdigit('x')) {
        return 3;
    }
    if (INT_MAX != 2147483647) {
        return 4;
    }
    if (sizeof(size_t) != 8 || sizeof(uint32_t) != 4) {
        return 5;
    }
    n = 1;
    flag = true;
    assert(flag);
    assert(n + 1 == 2);
    printf("%s %lu\n", p, (unsigned long)n);
    free(p);
    if (!flag || false) {
        return 6;
    }
    return 0;
}
