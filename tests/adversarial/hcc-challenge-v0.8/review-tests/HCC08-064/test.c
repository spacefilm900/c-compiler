/* HCC08-064: GNU packed nested aligned object offsets
 * GCC C extensions; N1570 6.5, 6.7, 6.8 as applicable. Documented GNU feature combined with ordinary C semantics. */
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#define CHECK(x) do { if (!(x)) return __LINE__; } while(0)
struct I{char c;__attribute__((aligned(16))) int x;};struct __attribute__((packed)) O{char c;struct I i __attribute__((aligned(16)));};
int main(void){
struct O o={0};o.i.x=47;CHECK(offsetof(struct O,i)==16&&sizeof o==48&&o.i.x==47);
puts("ok");return 0;
}
