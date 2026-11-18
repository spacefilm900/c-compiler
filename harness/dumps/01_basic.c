/* Golden dump test: scalar arithmetic, implicit conversions and control
   flow. Expected dumps are beside this file; regenerate them with
   `python3 harness/run_dump_tests.py --update`. */
int putchar(int);

static long scale(short s, unsigned char u) {
    long total = s * u;
    total += 1;
    return total;
}

double mix(int i, float f) {
    double d = i + f;
    if (d > 10)
        d = d / 2;
    return d;
}

int main(void) {
    int sum = 0;
    for (int i = 0; i < 4; i++)
        sum += (int)scale((short)i, 3);
    while (sum > 20)
        sum--;
    putchar('0' + sum % 10);
    putchar('\n');
    return mix(sum, 1.5f) > 5.0 ? 0 : 1;
}
