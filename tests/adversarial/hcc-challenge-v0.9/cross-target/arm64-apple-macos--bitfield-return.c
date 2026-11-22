struct S {unsigned a:3;unsigned :0;_Alignas(16) int x;unsigned b:5;};
struct S f(struct S);int main(void){struct S x={.a=3,.x=11,.b=7};struct S y=f(x);return y.a!=4||y.x!=22||y.b!=8;}
