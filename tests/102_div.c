typedef struct {
    int quot;
    int rem;
} div_t;
typedef struct {
    long quot;
    long rem;
} ldiv_t;

div_t div(int, int);
ldiv_t ldiv(long, long);

int main(void) {
    div_t d = div(7, 3);
    if (d.quot != 2 || d.rem != 1) {
        return 1;
    }
    ldiv_t ld = ldiv(-10L, 3L);
    if (ld.quot != -3L || ld.rem != -1L) {
        return 2;
    }
    return 0;
}
