/* HCC08-015: matrix nested record, alignment 64
 * 6.7.2.1; 6.7.5; 6.5.16.1. Check physical offset, stride, copy, and frontend layout together. */
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#define CHECK(x) do { if (!(x)) return __LINE__; } while(0)
struct I { char lead; _Alignas(64) int v; };
struct O {char lead; struct I i[2][2]; char tail;};

int main(void){
struct O x[3]={0}, y={0}; x[1].i[0][0].v=23; x[1].i[1][1].v=71; x[1].tail=9; y=x[1]; CHECK(sizeof(struct O)==640); CHECK(_Alignof(struct O)==64); CHECK(offsetof(struct O,i[1][1].v)==512); CHECK((uintptr_t)&x[1].i[1][1].v%64==0); CHECK((unsigned char*)&x[1].i[1][1].v==(unsigned char*)&x[1]+512); CHECK((uintptr_t)&x[2]-(uintptr_t)&x[1]==640); CHECK(y.i[1][1].v==71 && y.tail==9 && x[1].i[0][0].v==23);
puts("ok");return 0;
}
