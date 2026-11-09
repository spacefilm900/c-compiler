extern int twice(int n);

static int bias = 3;

int twice(int n) {
    return n + n + bias;
}

int main(void) {
    return twice(4); /* 11 */
}
