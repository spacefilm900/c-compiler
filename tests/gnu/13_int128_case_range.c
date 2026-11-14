/* GNU C: a case range whose span does not fit in 64 bits must still
   match every value from low to high inclusive. */

int printf(const char *, ...);

static int in_low(__int128 x)
{
    switch (x) {
    case 0 ... ((__int128)1 << 80):
        return 1;
    default:
        return 0;
    }
}

static int in_high(__int128 x)
{
    switch (x) {
    case ((__int128)1 << 70) ... ((__int128)1 << 80):
        return 1;
    default:
        return 0;
    }
}

int main(void)
{
    __int128 mid = (__int128)1 << 80;
    printf("%d %d %d\n", in_low(0), in_low(mid), in_low(mid + 1));
    printf("%d %d %d %d\n",
           in_high(((__int128)1 << 70) - 1),
           in_high((__int128)1 << 70),
           in_high(mid),
           in_high(mid + 1));
    return 0;
}
