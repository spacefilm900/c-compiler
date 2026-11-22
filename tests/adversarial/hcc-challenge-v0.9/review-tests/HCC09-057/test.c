#include <stdio.h>
#include <stdlib.h>
#define CK(x) do { if (!(x)) exit(1); } while (0)
#define OK() do { puts("ok"); return 0; } while (0)
#include <stddef.h>
#include <stdint.h>
#define A 8
struct S{unsigned a:3; unsigned :0; _Alignas(A) int x; unsigned b:5;};struct W{char c;struct S s;char tail;};int main(void){struct W w[3]={0};w[1].s.a=5;w[1].s.b=23;w[1].s.x=71;w[1].tail=9;w[2]=w[1];CK(_Alignof(struct S)==A && offsetof(struct S,x)%A==0);CK((uintptr_t)&w[1].s%A==0 && (uintptr_t)&w[2].s.x%A==0);CK(w[2].s.a==5 && w[2].s.b==23 && w[2].s.x==71 && w[2].tail==9 && w[0].s.x==0);OK();}
