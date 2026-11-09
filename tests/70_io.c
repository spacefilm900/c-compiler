int puts(const char *);
int putchar(int);
int printf(const char *, ...);

int main(void) {
    puts("hello");
    putchar('A');
    putchar('\n');
    putchar(48);
    printf("\n");
    return 0;
}
