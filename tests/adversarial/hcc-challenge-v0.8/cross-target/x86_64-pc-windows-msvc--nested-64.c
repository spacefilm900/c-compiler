/* x86_64-pc-windows-msvc--nested-64: 6.7.2.1; 6.7.5; 6.5.16.1. Target-specific contract; runtime check returns 0. */
#define EXPECT_LONG 4
#define EXPECT_LD 8
#define EXPECT_LDA 8
struct I {char c;_Alignas(64) int v;};union U{struct I i;char c;};struct O {char c;union U u;char end;};
_Static_assert(_Alignof(struct O)==64,"align");_Static_assert(sizeof(struct O)==256,"size");_Static_assert(__builtin_offsetof(struct O,u.i.v)==128,"offset");
int main(void){struct O x[2]={0},y;x[1].u.i.v=43;x[1].end=9;y=x[1];return y.u.i.v!=43||y.end!=9||(unsigned long long)&x[1].u.i.v-(unsigned long long)&x[1]!=128||(unsigned long long)&x[1].u.i.v%64!=0;}
