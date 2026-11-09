static int hidden_fun(void) {
    return 2;
}

int add(int a, int b) {
    return a + b;
}

int use_hidden(void) {
    int hidden_fun(void);
    return hidden_fun();
}

int shadow_fun(void) {
    int add = 3;
    if (add == 3) {
        extern int add(int, int);
        return add(4, 5);
    }
    return 0;
}

int main(void) {
    int id(int);
    int foo(void);
    int foo(void);
    int n = id(6);
    if (n != 6)
        return 1;
    if (use_hidden() != 2)
        return 2;
    if (shadow_fun() != 9)
        return 3;
    return foo() - 4;
}

int id(int x) {
    return x;
}

int foo(void) {
    return 4;
}
