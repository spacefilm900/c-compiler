int main(void) {
    int x = 0;
    int y = 0;
    int a = 1 && (x = 2);
    int b = 0 && (x = 9);
    int c = 0 || (y = 3);
    int d = 1 || (y = 8);
    int e = 0 || 1 && 0;
    int f = 1 && 1 && 1;
    /* x=2, y=3, a=1, b=0, c=1, d=1, e=0, f=1 */
    return x + y + a + b + c + d + e + f;
}
