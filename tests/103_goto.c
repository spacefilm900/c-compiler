void typedef_label(void) {
    typedef int T;
    T:
    return;
}

int main(void) {
    typedef_label();
    int n = 0;
    int i = 0;
start:
    if (i == 3) {
        goto done;
    }
    n += i;
    i += 1;
    goto start;
done:
    if (n != 3) {
        return 1;
    }
    goto mid;
    return 2;
mid:
    switch (n) {
    case 3:
        goto after;
    default:
        return 3;
    }
after:
    return 0;
}
