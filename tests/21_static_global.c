static int count;

static void bump(void) {
    count = count + 1;
}

static int hidden(int x) {
    return x + count;
}

int main(void) {
    bump();
    bump();
    bump();
    return hidden(4); /* 7 */
}
