int main(void) {
    int x = 1;
    int s = 0;
    for (int i = 0; i < 3; i = i + 1) {
        int x = i;
        s = s + x;
    }
    return s + x; /* 4 */
}
