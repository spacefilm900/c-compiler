enum Color { RED = 1, GREEN, BLUE = GREEN + 3 };

int main(void) {
    enum Color c = GREEN;
    enum { TEN = 10, ELEVEN };
    int n = RED;
    if (c != 2) {
        return 1;
    }
    if (BLUE != 5) {
        return 2;
    }
    if (ELEVEN != 11) {
        return 3;
    }
    return n + c + BLUE + TEN;
}
