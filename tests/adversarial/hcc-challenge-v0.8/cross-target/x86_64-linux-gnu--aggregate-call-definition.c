/* Separate translation unit. C11 6.5.2.2; 6.9.1. */
struct S{char c;_Alignas(32) long long n;int z;};
struct S transform(struct S s,int n){s.n+=n;s.z++;return s;}
