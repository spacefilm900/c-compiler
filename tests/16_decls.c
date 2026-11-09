int main(void) {
    int a = 1, b = a + 4;
    int s = 0;
    for (int i = 1, j = 2; i < 4; i = i + 1)
        s = s + j;
    return a + b + s; /* 12 */
}
