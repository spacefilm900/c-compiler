int pick(int n) {
    if (n == 0)
        return 1;
    return n + pick(n - 1);
}

int main(void) {
    return pick(5); /* 16 */
}
