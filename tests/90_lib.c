#include <stddef.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

int main(void) {
    char *p = NULL;
    if (p != 0) {
        return 1;
    }
    if (strcmp("ab", "ab") != 0) {
        return 2;
    }
    if (strlen("hi") != 2) {
        return 3;
    }
    printf("%d\n", abs(-4));
    return 0;
}
