#include <stdnoreturn.h>

noreturn void stop(void);

int main(void) {
    return 0;
}

noreturn void stop(void) {
    for (;;) {
    }
}
