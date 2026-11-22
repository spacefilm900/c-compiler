/* arm64-apple-macos--nested-16: 6.7.2.1; 6.7.5; target ABI. Target-specific contract; runtime check returns 0. */
#define EXPECT_LONG 8
#define EXPECT_LD 8
#define EXPECT_LDA 8
struct I {unsigned b:3;_Alignas(16) int x;};struct O{char c;struct I a[2];char z;};
_Static_assert(_Alignof(struct O)==16,"align");_Static_assert(sizeof(struct O)==96,"size");_Static_assert(__builtin_offsetof(struct O,a[1].x)==64,"offset");
int main(void){struct O a[2]={0};a[1].a[1].b=5;a[1].a[1].x=37;return a[1].a[1].b!=5||a[1].a[1].x!=37||(unsigned long long)&a[1].a[1].x%16!=0||(unsigned long long)&a[1].a[1].x-(unsigned long long)&a[1]!=64;}
