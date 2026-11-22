/* HCC07-058: Packed record nested with explicit aligned member
 * Basis: N1570 6.7.2.1; GCC manual Common Type Attributes
 * Packed non-bitfield layout and explicit higher member alignment affect offset and stride; contract checked on Linux.
 * Expected: exit 0; stdout: ok; mode: gnu11.
 */
#include <stdio.h>
#define CHECK(x) do { if (!(x)) return 1; } while (0)
#include <stddef.h>
struct __attribute__((packed)) R { char c; int i; int j __attribute__((aligned(8))); };
int main(void) {
struct R a[2]; a[1].i=0x1234; a[1].j=19; CHECK(offsetof(struct R,i)==1 && offsetof(struct R,j)%8==0 && a[1].i==0x1234 && a[1].j==19);
    puts("ok"); return 0;
}
