/* A hidden block-scope extern is not a visible prior declaration
   (C11 6.2.7p4). A later file-scope `int a[]` is completed by its
   initializer, not by the hidden size. Within that initializer the
   identifier has this declarator's type, so `a` is incomplete
   `int[]`. `sizeof a[0]` is valid because the element type is
   complete; `sizeof a` is a diagnostic and is tested in src/sema.rs.
   Hidden and later types in this file are compatible (6.2.7p2). */

int fa(void) {
    extern int a[1];
    return 0;
}
int a[] = { [0 && sizeof a[0]] = 7 };

int fb(void) {
    extern int b[3];
    return 0;
}
int b[] = {1, 2, 3};

int fc(void) {
    extern int c[];
    return 0;
}
int c[] = {1, 2};

int fd(void) {
    extern int d[1];
    return 0;
}
/* This declarator is complete, so `sizeof d` is valid (6.5.3.4p1). */
int d[1] = { [0 && sizeof d] = 9 };

int main(void) {
    if (sizeof a != sizeof(int) || a[0] != 7)
        return 1;
    if (sizeof b != 3 * sizeof(int) || b[0] != 1 || b[1] != 2 || b[2] != 3)
        return 2;
    if (sizeof c != 2 * sizeof(int) || c[0] != 1 || c[1] != 2)
        return 3;
    if (sizeof d != sizeof(int) || d[0] != 9)
        return 4;
    return fa() + fb() + fc() + fd();
}
