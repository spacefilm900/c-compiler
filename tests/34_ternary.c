int calls;

void tick(void) {
    calls = calls + 1;
}

int id(int n) {
    calls = calls + 1;
    return n;
}

int main(void) {
    int x = 0;
    int y = 1 ? 2 : (x = 9);
    int z = 0 ? (x = 8) : 4;
    int right = 1 ? 0 ? 10 : 20 : 30;
    int right0 = 0 ? 1 ? 2 : 3 : 4;
    int loose = 1 ? 2 : 3 + 4;
    int loose0 = 0 ? 1 : 2 + 3;
    int neg = 1 ? -3 : 4;
    int picked;
    if (y != 2 || x != 0)
        return 1;
    if (z != 4 || x != 0)
        return 2;
    if (right != 20)
        return 3;
    if (right0 != 4)
        return 4;
    if (loose != 2)
        return 5;
    if (loose0 != 5)
        return 6;
    if (neg != -3)
        return 7;
    calls = 0;
    1 ? tick() : tick();
    if (calls != 1)
        return 8;
    0 ? tick() : tick();
    if (calls != 2)
        return 9;
    calls = 0;
    picked = 0 ? id(8) : id(6);
    if (picked != 6 || calls != 1)
        return 10;
    picked = calls ? 1 : 2;
    if (picked != 1)
        return 11;
    return 0;
}
