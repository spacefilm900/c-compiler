/* x86_64-linux-gnu--aggregate-call: 6.5.2.2; 6.9.1; target ABI. Target-specific contract; runtime check returns 0. */
#define EXPECT_LONG 8
#define EXPECT_LD 16
#define EXPECT_LDA 16
struct S{char c;_Alignas(32) long long n;int z;};
struct S transform(struct S,int);
int main(void){struct S x={3,71,8};struct S y=transform(x,19);return y.c!=3||y.n!=90||y.z!=9||x.n!=71;}
