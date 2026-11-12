/* C11 6.7.3p2 allows restrict only on pointers to object types. A
   function may return a restrict-qualified pointer and take
   restrict-qualified pointer parameters; the qualifier on the return
   type does not change the value returned (6.7.3p4, 6.5.2.2p5). */

static int store[4];

static int *restrict pick(int *restrict base, int i) {
    return base + i;
}

static void copy(int *restrict dst, const int *restrict src, int n) {
    for (int i = 0; i < n; i++)
        dst[i] = src[i];
}

int main(void) {
    int *restrict (*fp)(int *restrict, int) = pick;
    int src[4] = {3, 5, 7, 11};
    copy(store, src, 4);
    *pick(store, 1) += 10;
    *fp(store, 3) *= 2;
    {
        int *restrict local(int *restrict, int);
        if (local(store, 2) != store + 2)
            return 9;
    }
    return store[0] + store[1] + store[2] + store[3];
}

int *restrict local(int *restrict base, int i) {
    return base + i;
}
