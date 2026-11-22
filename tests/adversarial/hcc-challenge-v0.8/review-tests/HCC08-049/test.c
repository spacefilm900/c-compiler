/* HCC08-049: VLA with aligned record stride
 * 6.2.4; 6.5.3.4; 6.7.6.2; 6.7.6.3; 6.8.6. Defined size, indexing, lifetime or control-flow invariant. */
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#define CHECK(x) do { if (!(x)) return __LINE__; } while(0)

int main(void){
int n=5;struct I{char c;_Alignas(32) int v;};struct O{char c;struct I i;};struct O a[n];for(int i=0;i<n;i++){a[i].i.v=i+10;CHECK((uintptr_t)&a[i].i.v%32==0);}CHECK(sizeof(a)==(size_t)n*sizeof(struct O));CHECK(a[4].i.v==14);
puts("ok");return 0;
}
