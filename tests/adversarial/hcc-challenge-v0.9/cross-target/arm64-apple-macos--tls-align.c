struct S{_Alignas(32) int a[8];};static _Thread_local struct S x;int main(void){x.a[7]=71;return (unsigned long long)&x%32!=0||x.a[0]!=0||x.a[7]!=71;}
