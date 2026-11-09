struct s;
struct s *p;

int main(void) {
    p = 0;
    /* &*p does not load through an incomplete struct type. */
    return &*p == p;
}
