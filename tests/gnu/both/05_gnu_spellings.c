/* Alternate keyword spellings GCC accepts in every mode (GCC manual,
   "Alternate Keywords") and the __FUNCTION__ name. */

int printf(const char *, ...);

static __inline__ int add1(int v) { return v + 1; }
static __inline int add2(int v) { return v + 2; }

int main(void)
{
    __signed__ char sc = -3;
    __const int ci = 4;
    __const__ int cj = 5;
    __volatile__ int vi = 6;
    __volatile int vj = 7;
    int *__restrict__ rp = &vi + 0 == 0 ? 0 : (int *)&vi;
    __extension__ long long big = 1LL << 40;
    printf("%d %d %d %d %d %lld\n", sc, ci, cj, vi, vj, big);
    printf("%zu %zu %zu\n", __alignof__(double), __alignof(long), __alignof__(ci));
    printf("%s %s\n", __FUNCTION__, __func__);
    printf("%d %d %d\n", add1(1), add2(1), rp != 0);
    return 0;
}
