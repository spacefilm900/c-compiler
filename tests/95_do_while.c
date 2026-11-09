int side;

int bump(void) {
    side = side + 1;
    return 1;
}

int nested(void) {
    int s = 0;
    int i = 0;
    do {
        int j = 0;
        do {
            j = j + 1;
            if (j == 1)
                continue;
            s = s + 1;
        } while (j < 3);
        i = i + 1;
    } while (i < 2);
    return s;
}

int main(void) {
    int a = 1;
    int b = 10;
    int i = 0;
    int n = 0;
    int z = 2147483642;
    int sum = 0;
    int k = 4;
    do {
        a = a * 2;
    } while (a < 11);
    if (a != 16)
        return 1;

    do
        break;
    while (bump());
    if (b != 10 || side != 0)
        return 2;

    do {
        i = i + 1;
        if (i < 3)
            continue;
        n = n + 1;
    } while (i < 4);
    if (n != 2 || i != 4)
        return 3;

    do
        ;
    while ((z = z - 5) >= 256);
    if (z != 252)
        return 4;

    do
        sum = sum + 2;
    while (k = k - 1);
    if (sum != 8 || k != 0)
        return 5;

    if (nested() != 4)
        return 6;
    return 0;
}
