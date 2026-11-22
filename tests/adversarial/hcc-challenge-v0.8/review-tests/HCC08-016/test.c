/* HCC08-016: Flexible array element alignment 64
 * 6.7.2.1p18; 6.7.5. Target layout of flexible array header; no flexible element accessed. */
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#define CHECK(x) do { if (!(x)) return __LINE__; } while(0)
struct I { char lead; _Alignas(64) int v; };
struct O { char lead; struct I i[]; };

int main(void){
struct O o; CHECK(offsetof(struct O,i)==64); CHECK(sizeof(struct O)==64); CHECK(_Alignof(struct O)==64); CHECK((uintptr_t)&o.i%64==0);
puts("ok");return 0;
}
