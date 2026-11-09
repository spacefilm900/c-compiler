void bump(void);
int add(int n);

int count;

int main(void) {
    bump();
    bump();
    return add(3); /* 5 */
}

void bump(void) {
    count = count + 1;
}

int add(int n) {
    count = count + n;
    return count;
}
