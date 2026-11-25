/* N1570 5.1.2.2 hosted environment: main, argv, return from main */
#include "check.h"

int main(int argc, char *argv[]) {
    CHECK(argc >= 1);
    CHECK(argv[argc] == 0);
    CHECK(argv[0] != 0);
    if (check_failures)
        return 1;
    printf("done 0\n");
    /* 5.1.2.2.3: reaching the } that terminates main returns 0 */
}
