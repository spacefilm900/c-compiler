#include <stdbool.h>

int main(void) {
    _Bool b = 2;
    bool f = false;
    bool t = true;
    int n = 0;
    if (b != 1) {
        return 1;
    }
    if (f || !t) {
        return 2;
    }
    if ((_Bool)n != 0) {
        return 3;
    }
    if ((_Bool)256 != 1) {
        return 4;
    }
    return (int)b + (int)t;
}
