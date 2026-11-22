struct S {unsigned a:3;unsigned :0;_Alignas(16) int x;unsigned b:5;};
struct S f(struct S x){x.a++;x.x*=2;x.b++;return x;}
