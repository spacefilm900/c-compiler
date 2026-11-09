int calls;

void bump(void) {
    calls = calls + 1;
}

int add(int a, int b) {
    return a + b;
}

int main(void) {
    int x = 1;
    int y = (x = 4, x + 3);
    int a = 1, b = 2;
    int i;
    int j;
    int s = 0;
    int from_args;
    int from_comma;
    if (y != 7 || x != 4)
        return 1;
    if (a + b != 3)
        return 2;
    for (i = 0, j = 1; i < 3; i++, j++)
        s = s + i + j;
    if (s != 9)
        return 3;
    from_args = add(1, 2);
    from_comma = add((1, 5), 2);
    if (from_args != 3 || from_comma != 7)
        return 4;
    calls = 0;
    x = (bump(), bump(), 6);
    if (x != 6 || calls != 2)
        return 5;
    (bump(), bump());
    if (calls != 4)
        return 6;
    if (add(1 ? 2 : 9, 4) != 6)
        return 7;
    {
        int pair = (1, 2);
        if (pair != 2)
            return 8;
    }
    return (0, 0);
}
