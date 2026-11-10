long double addld(long double a, long double b);
long double mixld(int n, long double x, int m);

int main(void) {
    long double sum = addld(1.5L, 2.5L);
    long double mixed = mixld(2, 3.5L, 4);
    if (sum != 4.0L) {
        return 1;
    }
    if (mixed != 9.5L) {
        return 2;
    }
    return 0;
}
