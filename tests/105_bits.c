#include <string.h>

struct A {
    int a : 3;
    int b : 5;
};

struct C {
    int a : 3;
    char b;
};

struct E {
    int a : 20;
    int b : 20;
};

struct U {
    char a : 3;
    int : 0;
    char b;
};

int main(void) {
    struct A a = {7, 31};
    if (a.a != -1 || a.b != -1) {
        return 1;
    }
    a.a = 1;
    a.b = 2;
    if (a.a != 1 || a.b != 2) {
        return 2;
    }
    unsigned char bytes[4];
    memcpy(bytes, &a, 4);
    /* bits 0-2 = 1, bits 3-7 = 2 -> 0x11 */
    if (bytes[0] != 0x11 || bytes[1] != 0 || bytes[2] != 0 || bytes[3] != 0) {
        return 3;
    }
    struct C c = {.a = 1, .b = 0x22};
    if (c.a != 1 || c.b != 0x22) {
        return 4;
    }
    if (sizeof(struct C) != 4) {
        return 5;
    }
    struct E e = {1, 1};
    if (e.a != 1 || e.b != 1 || sizeof(struct E) != 8) {
        return 6;
    }
    struct U u = {1, 0x22};
    if (u.a != 1 || u.b != 0x22 || sizeof(struct U) != 5) {
        return 7;
    }
    struct {
        unsigned x : 3;
    } u3;
    u3.x = 7;
    if (u3.x != 7 || !(u3.x > -1)) {
        return 8;
    }
    struct {
        int x : 1;
    } s1 = {1};
    if (s1.x != -1) {
        return 9;
    }
    return 0;
}
