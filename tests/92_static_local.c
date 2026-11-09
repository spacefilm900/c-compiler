int foo(void) {
    static int a = 3;
    a = a * 2;
    return a;
}

int bar(void) {
    static int a = 4;
    a = a + 1;
    return a;
}

int nested(void) {
    static int outer = 1;
    int seen = outer;
    outer = outer + 1;
    {
        static int inner = 10;
        int value = inner;
        inner = inner + 1;
        return seen * 100 + value;
    }
}

int zeroed(void) {
    static int x;
    x = x + 1;
    return x;
}

struct Pair {
    int x;
    int y;
};

int acc(int x, int y) {
    static struct Pair s;
    int old = s.x + s.y;
    s.x = x;
    s.y = y;
    return old;
}

int letters(void) {
    static char s[] = "hi";
    return s[0] + s[1] + s[2];
}

int main(void) {
    static int kept = 7;
    if (foo() + bar() != 11)
        return 1;
    if (foo() + bar() != 18)
        return 2;
    if (nested() != 110)
        return 3;
    if (nested() != 211)
        return 4;
    if (zeroed() != 1)
        return 5;
    if (zeroed() != 2)
        return 6;
    if (acc(1, 2) != 0)
        return 7;
    if (acc(3, 4) != 3)
        return 8;
    if (letters() != 209)
        return 9;
    kept = kept + 1;
    return kept - 8;
}
