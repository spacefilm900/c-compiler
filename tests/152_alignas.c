/* _Alignas (C11 6.7.5) on objects and members: layout, sizeof, _Alignof,
   and the object's address alignment. */
int printf(const char *, ...);

_Alignas(16) int g;
_Alignas(int) char c;
struct S {
    char a;
    _Alignas(8) int b;
    _Alignas(16) char d;
};
_Static_assert(_Alignof(struct S) == 16, "S align");
_Static_assert(sizeof(struct S) == 32, "S size");

int main(void) {
    _Alignas(32) char buf[4];
    struct S s;
    unsigned long ga = (unsigned long)&g;
    unsigned long ba = (unsigned long)&buf;
    unsigned long sa = (unsigned long)&s;
    unsigned long sb = (unsigned long)&s.b;
    unsigned long sd = (unsigned long)&s.d;
    printf("%d %d %d %d %d %d %d %d %d\n",
           (int)sizeof(g), (int)_Alignof(int), (int)sizeof(struct S),
           (int)_Alignof(struct S), (int)sizeof(buf),
           (int)(ga % 16), (int)(ba % 32), (int)(sb % 8), (int)(sd % 16));
    return (int)(sa % 16);
}
