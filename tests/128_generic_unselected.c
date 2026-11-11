/* Unselected _Generic associations are not evaluated (C11 6.5.1.1p3).
   Side effects in those expressions must not run. Valid unselected
   expressions may have a type different from the selected one. */
int printf(const char *, ...);
static int bumped;
static int bump(void) {
    bumped++;
    return 99;
}
int main(void) {
    int n = 0;
    int y = 4;
    int z = 5;
    int chosen = _Generic(n, int: 3, default: bump());
    int inc = _Generic(n, int: 7, default: ++y);
    int asg = _Generic(n, int: 8, default: (z = 0));
    int pr = _Generic(n, int: 9, default: printf("unselected\n"));
    double d = _Generic(n, int: 1.5, default: 2);
    const char *s = _Generic(n, int: "sel", long: "other");
    if (chosen != 3 || inc != 7 || asg != 8 || pr != 9) {
        return 1;
    }
    if (bumped != 0 || y != 4 || z != 5) {
        return 2;
    }
    if (d != 1.5 || s[0] != 's') {
        return 3;
    }
    /* C11 6.4.2.2: in main, __func__ is a static const char[5]. */
    enum { FuncWrong = _Generic(&__func__, char (*)[99]: 1, default: 0) };
    enum { FuncRight = _Generic(&__func__, const char (*)[5]: 1, default: 0) };
    int func_rt = _Generic(&__func__, const char (*)[5]: 1, default: 0);
    int desig = (int[1]){[sizeof 0 - sizeof 0] = 7}[0];
    int unsel = _Generic(0, int: 3, default: (int[1]){[sizeof 0 - sizeof 0] = 0});
    /* C11 6.2.1p7: the object is in scope in its initializer. */
    int self[1] = { [0 && sizeof self] = 7 };
    if (FuncWrong != 0 || FuncRight != 1 || func_rt != 1) {
        return 4;
    }
    if (desig != 7 || unsel != 3) {
        return 5;
    }
    if (self[0] != 7) {
        return 6;
    }
    printf("%d %d %d\n", chosen, bumped, y);
    return 0;
}
