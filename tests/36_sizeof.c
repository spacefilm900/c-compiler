int calls;

int tick(void) {
    calls = calls + 1;
    return 9;
}

int n = sizeof(tick());
int h = 3;
int g = sizeof(h++);
int self = sizeof(self);

int main(void) {
    int x = 1;
    int y = sizeof(x = 5);
    int z = sizeof(x++);
    int w = sizeof(int);
    int v = sizeof x;
    int u = sizeof(x + 1);
    if (y != 4 || x != 1)
        return 1;
    if (z != 4 || x != 1)
        return 2;
    if (w != 4 || v != 4 || u != 4)
        return 3;
    if (n != 4 || calls != 0)
        return 4;
    if (h != 3 || g != 4)
        return 5;
    if (self != 4)
        return 6;
    if (sizeof(1, tick()) != 4 || calls != 0)
        return 7;
    return 0;
}
