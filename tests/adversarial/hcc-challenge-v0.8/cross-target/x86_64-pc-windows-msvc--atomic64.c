/* x86_64-pc-windows-msvc--atomic64: 6.5.16.2p3; 6.5.2.4; 6.7.2.4. Target-specific contract; runtime check returns 0. */
#define EXPECT_LONG 4
#define EXPECT_LD 8
#define EXPECT_LDA 8
_Atomic(unsigned long long) x=7;int main(void){unsigned long long y=(x+=5);return y!=12||x!=12||x--!=12||x!=11;}
