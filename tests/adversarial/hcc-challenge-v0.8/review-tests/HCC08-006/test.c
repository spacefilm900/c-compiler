/* HCC08-006: bitfield nested record, alignment 16
 * 6.7.2.1; 6.7.5; 6.5.16.1. Check physical offset, stride, copy, and frontend layout together. */
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#define CHECK(x) do { if (!(x)) return __LINE__; } while(0)
struct I { unsigned int b:3; _Alignas(16) int v; }; struct O {char lead; struct I i; char tail;};

int main(void){
struct O x[3]={0}, y={0}; x[1].i.b=5; x[1].i.v=71; x[1].tail=9; y=x[1]; CHECK(sizeof(struct O)==64); CHECK(_Alignof(struct O)==16); CHECK(offsetof(struct O,i.v)==32); CHECK((uintptr_t)&x[1].i.v%16==0); CHECK((unsigned char*)&x[1].i.v==(unsigned char*)&x[1]+32); CHECK((uintptr_t)&x[2]-(uintptr_t)&x[1]==64); CHECK(y.i.v==71 && y.tail==9 && x[1].i.b==5);
puts("ok");return 0;
}
