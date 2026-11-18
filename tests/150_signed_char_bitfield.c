/* A bit-field declared `signed char` is signed on every target, also where
   plain `char` is unsigned (AArch64 Linux). Its value is sign-extended when
   it is read (C11 6.7.2.1p10, 6.3.1.1p2), and stores keep the neighbours. */
int printf(const char *, ...);

struct S { signed char a : 3; signed char b : 5; unsigned char c : 4; signed char d : 4; };

int main(void) {
    struct S s = { -2, -9, 15, 7 };
    printf("%d %d %d %d\n", s.a, s.b, s.c, s.d);
    s.a += 1;
    s.b *= 1.0;
    s.d -= 15;
    printf("%d %d %d %d\n", s.a, s.b, s.c, s.d);
    int sum = s.a + s.b + s.d;
    return sum == -1 - 9 - 8 ? 0 : 1;
}
