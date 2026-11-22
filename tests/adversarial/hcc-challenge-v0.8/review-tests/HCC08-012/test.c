/* HCC08-012: Flexible array element alignment 32
 * 6.7.2.1p18; 6.7.5. Target layout of flexible array header; no flexible element accessed. */
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#define CHECK(x) do { if (!(x)) return __LINE__; } while(0)
struct I { char lead; _Alignas(32) int v; };
struct O { char lead; struct I i[]; };

int main(void){
struct O o; CHECK(offsetof(struct O,i)==32); CHECK(sizeof(struct O)==32); CHECK(_Alignof(struct O)==32); CHECK((uintptr_t)&o.i%32==0);
puts("ok");return 0;
}
