struct LD {
    long double x;
};

struct LD idld(struct LD s);
struct LD makeld(long double v);

int main(void) {
    struct LD s = makeld(1.25L);
    if (s.x != 1.25L) {
        return 1;
    }
    s = idld(s);
    if (s.x != 1.25L) {
        return 2;
    }
    return 0;
}
