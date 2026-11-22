/* HCC07-031: VLA element type containing bit-fields
 * Basis: 6.7.6.2; 6.7.2.1
 * Runtime stride is record size, including bit-fields and trailing member.
 * Expected: exit 0; stdout: ok; mode: c11.
 */
#include <stdio.h>
#define CHECK(x) do { if (!(x)) return 1; } while (0)

struct R { unsigned x:3; unsigned y:5; int z; };
int main(void) {
int n=4; struct R a[n]; for(int i=0;i<n;i++){a[i].x=i;a[i].y=i+8;} CHECK(a[3].x==3 && a[3].y==11 && sizeof a==4*sizeof(struct R));
    puts("ok"); return 0;
}
