void add(int n);

int total;

int main(void) {
    add(4);
    add(5);
    return total; /* 9 */
}

void add(int n) {
    total = total + n;
}

void add(int);
