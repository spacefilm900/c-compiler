#include <stdio.h>
#include <stdlib.h>
#define CK(x) do { if (!(x)) exit(1); } while (0)
#define OK() do { puts("ok"); return 0; } while (0)
struct S{_Alignas(16) int v;unsigned b:3;};int main(void){int n=3;typedef struct S Row[n];Row a;for(int i=0;i<n;i++){a[i].v=i*2;a[i].b=i%8;}Row *p=&a;CK(sizeof *p==sizeof(struct S)*(unsigned)n);for(int i=0;i<n;i++)CK((*p)[i].v==i*2 && (*p)[i].b==(unsigned)i%8);OK();}
