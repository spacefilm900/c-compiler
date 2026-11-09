int main(void) {
    int a = __LINE__;
    int b =
        __LINE__;
    if (a != 2) {
        return 1;
    }
    if (b != 4) {
        return b;
    }
    return 0;
}
