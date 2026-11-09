int x = 10;

int inner(void) {
    int x = 4;
    return x;
}

int outer(void) {
    return x;
}

int main(void) {
    int x = 1;
    {
        int y = x;
        x = 3;
        return inner() + outer() + x + y; /* 18 */
    }
}
