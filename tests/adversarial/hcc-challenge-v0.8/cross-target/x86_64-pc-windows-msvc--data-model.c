/* x86_64-pc-windows-msvc--data-model: 6.2.5; 6.2.8; 6.7.2.1; target ABI. Target-specific contract; runtime check returns 0. */
#define EXPECT_LONG 4
#define EXPECT_LD 8
#define EXPECT_LDA 8
_Static_assert(sizeof(void*)==8,"pointer size");
_Static_assert(sizeof(int)==4,"int size");
_Static_assert(sizeof(long)==EXPECT_LONG,"long size");
_Static_assert(sizeof(long double)==EXPECT_LD,"long double size");
_Static_assert(_Alignof(long double)==EXPECT_LDA,"long double alignment");
struct S {char c;long double x;char z;};
_Static_assert(__builtin_offsetof(struct S,x)==EXPECT_LDA,"member offset");
_Static_assert(sizeof(struct S)==3*EXPECT_LDA,"record size");
int main(void){struct S a[2];a[1].x=7.5L;return a[1].x!=7.5L || (unsigned long long)&a[1].x-(unsigned long long)&a[1]!=EXPECT_LDA;}
