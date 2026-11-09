#define FIRST(a, ...) a
#define REST(a, ...) __VA_ARGS__
#define SUM(a, b, c) ((a) + (b) + (c))
#define WRAP(x) SUM(x)

int main(void) {
    return WRAP(REST(9, 1, 2, 4)) + FIRST(3, 4, 5);
}
