int main(void) {
    int s = 0;
    int i = 0;
    while (i < 10) {
        i = i + 1;
        if (i == 3)
            continue;
        if (i == 8)
            break;
        s = s + i;
    }
    return s; /* 25 */
}
