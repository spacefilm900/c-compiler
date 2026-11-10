struct LD {
    long double x;
};

struct LD idld(struct LD s) {
    return s;
}

struct LD makeld(long double v) {
    struct LD s;
    s.x = v;
    return s;
}
