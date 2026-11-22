/* HCC08-005: union nested record, alignment 16
 * 6.7.2.1; 6.7.5; 6.5.16.1. Check physical offset, stride, copy, and frontend layout together. */
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#define CHECK(x) do { if (!(x)) return __LINE__; } while(0)
struct I { char lead; _Alignas(16) int v; };
union U { struct I i; char c; }; struct O {char lead; union U u; char tail;};

int main(void){
struct O x[3]={0}, y={0}; x[1].u.i.lead=3; x[1].u.i.v=71; x[1].tail=9; y=x[1]; CHECK(sizeof(struct O)==64); CHECK(_Alignof(struct O)==16); CHECK(offsetof(struct O,u.i.v)==32); CHECK((uintptr_t)&x[1].u.i.v%16==0); CHECK((unsigned char*)&x[1].u.i.v==(unsigned char*)&x[1]+32); CHECK((uintptr_t)&x[2]-(uintptr_t)&x[1]==64); CHECK(y.u.i.v==71 && y.tail==9 && x[1].u.i.lead==3);
puts("ok");return 0;
}
