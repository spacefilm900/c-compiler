/* HCC08-004: Flexible array element alignment 8
 * 6.7.2.1p18; 6.7.5. Target layout of flexible array header; no flexible element accessed. */
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#define CHECK(x) do { if (!(x)) return __LINE__; } while(0)
struct I { char lead; _Alignas(8) int v; };
struct O { char lead; struct I i[]; };

int main(void){
struct O o; CHECK(offsetof(struct O,i)==8); CHECK(sizeof(struct O)==8); CHECK(_Alignof(struct O)==8); CHECK((uintptr_t)&o.i%8==0);
puts("ok");return 0;
}
