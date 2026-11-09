int printf(const char *, ...);

int main(void) {
    printf("|%5d|\n", 42);
    printf("|%-5d|\n", 42);
    printf("|%05d|\n", 42);
    printf("|%+d|\n", 42);
    printf("|% d|\n", 42);
    printf("|%.3d|\n", 42);
    printf("|%5.3d|\n", 42);
    printf("|%08x|\n", 0xabu);
    printf("|%5s|\n", "ab");
    printf("|%.2s|\n", "abcd");
    printf("|%-5.2s|\n", "abcd");
    printf("|%+5d|\n", -7);
    return 0;
}
