int printf(const char *, ...);

struct R {
    double d __attribute__((aligned(16)));
};
struct Q {
    float f;
} __attribute__((aligned(16)));

double take_aligned(int n, ...);

int main(void) {
    struct R r = { 4.5 };
    struct Q q = { 7.25f };
    /* Eight doubles fill the XMM registers, so r, q and the doubles after
       them are on the stack. */
    double full = take_aligned(8, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, r, 9.5, q, 3.0, 11);
    /* With registers to spare the records come from XMM slots. */
    double few = take_aligned(2, 1.0, 2.0, r, 9.5, q, 3.0, 12);
    /* Seven doubles: r takes the last XMM register, q is on the stack. */
    double seven = take_aligned(7, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, r, 9.5, q, 3.0, 13);
    printf("%.2f %.2f %.2f\n", full, few, seven);
    return full == 8000.0 + 450.0 + 95.0 + 7.25 + 1.5 + 11 ? 0 : 1;
}
