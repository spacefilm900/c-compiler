/* A record that is one SSE eightbyte but 16-byte aligned (System V AMD64
   classifies it SSE, NO_CLASS) is taken from the overflow area with its own
   alignment and its 16-byte size once the eight XMM registers are used up.
   Before the fix the overflow cursor moved by 8 bytes, so the arguments
   after it were read from the wrong place. */

struct R {
    double d __attribute__((aligned(16)));
};
struct Q {
    float f;
} __attribute__((aligned(16)));

double take_aligned(int n, ...) {
    __builtin_va_list ap;
    __builtin_va_start(ap, n);
    double total = 0.0;
    for (int i = 0; i < n; i++) {
        total += __builtin_va_arg(ap, double);
    }
    struct R r = __builtin_va_arg(ap, struct R);
    double after = __builtin_va_arg(ap, double);
    struct Q q = __builtin_va_arg(ap, struct Q);
    double after2 = __builtin_va_arg(ap, double);
    int tail = __builtin_va_arg(ap, int);
    __builtin_va_end(ap);
    return total * 1000.0 + r.d * 100.0 + after * 10.0 + q.f + after2 * 0.5 + tail;
}
