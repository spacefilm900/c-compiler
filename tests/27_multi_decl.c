int id(int a);
int id(int b);
int id(int);

int a = 2, b = 5;

int id(int x) {
    return x + a;
}

int main(void) {
    return id(b); /* 7 */
}
