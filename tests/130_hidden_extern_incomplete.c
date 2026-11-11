/* A hidden block-scope extern is not a visible prior declaration
   (C11 6.2.7p4). A later file-scope `int a[]` is completed by its
   initializer, not by the hidden size. */

int fa(void) {
    extern int a[1];
    return 0;
}
int a[] = {7};

int fb(void) {
    extern int b[1];
    return 0;
}
int b[] = {1, 2, 3};

int fc(void) {
    extern int c[3];
    return 0;
}
int c[] = {1, 2};

int main(void) {
    if (sizeof a != sizeof(int) || a[0] != 7)
        return 1;
    if (sizeof b != 3 * sizeof(int) || b[0] != 1 || b[1] != 2 || b[2] != 3)
        return 2;
    if (sizeof c != 2 * sizeof(int) || c[0] != 1 || c[1] != 2)
        return 3;
    return fa() + fb() + fc();
}
