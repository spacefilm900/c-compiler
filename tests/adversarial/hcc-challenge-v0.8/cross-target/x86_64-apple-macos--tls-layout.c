/* x86_64-apple-macos--tls-layout: 6.2.4p4; 6.7.1; 6.7.5. Target-specific contract; runtime check returns 0. */
#define EXPECT_LONG 8
#define EXPECT_LD 16
#define EXPECT_LDA 16
struct S{char c;_Alignas(32) int x;};_Thread_local struct S a[2];
int main(void){a[1].x=89;return a[0].x!=0||a[1].x!=89||(unsigned long long)&a[1].x%32!=0;}
