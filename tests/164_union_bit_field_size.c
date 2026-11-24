/* A bit-field in a union occupies only the bytes its width needs, not its
   declared type (observed with Clang and GCC as black boxes on x86-64 and
   arm64 Apple; C11 6.7.2.1p11 leaves the unit to the implementation). An
   unnamed bit-field adds to the size but not to the alignment, so
   `union { char a; int : 3; }` is 1 byte. hcc used the declared type's
   size and made it 4. */
int printf(const char *, ...);

union U2 { char a; int : 3; };
union U3 { char a : 2; long long : 7; };
union U4 { char a; int : 12; };
union U5 { int : 3; char a; };
union U6 { short a : 3; int : 20; };
union U7 { char a; long long : 33; };
union U8 { int a : 3; char c; };
struct S { char c; union U4 u; char d; };

union U4 u4 = {7};
struct S s = {1, {2}, 3};

int main(void) {
    printf("%d %d %d %d\n", (int)sizeof(union U2), (int)_Alignof(union U2), (int)sizeof(union U3),
           (int)_Alignof(union U3));
    printf("%d %d %d %d\n", (int)sizeof(union U4), (int)_Alignof(union U4), (int)sizeof(union U5),
           (int)_Alignof(union U5));
    printf("%d %d %d %d\n", (int)sizeof(union U6), (int)_Alignof(union U6), (int)sizeof(union U7),
           (int)_Alignof(union U7));
    printf("%d %d %d\n", (int)sizeof(union U8), (int)_Alignof(union U8), (int)sizeof(struct S));
    printf("%d %d %d %d\n", u4.a, s.c, s.u.a, s.d);
    return 0;
}
