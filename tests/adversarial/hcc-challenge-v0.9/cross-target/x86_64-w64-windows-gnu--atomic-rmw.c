_Atomic(unsigned int) x=7;_Atomic(unsigned long long) y=11;int main(void){unsigned a=x++; unsigned long long b=(y+=5);return a!=7||x!=8||b!=16||y!=16;}
