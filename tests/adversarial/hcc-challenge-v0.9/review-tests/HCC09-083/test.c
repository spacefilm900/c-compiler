#include <stdio.h>
#include <stdlib.h>
#define CK(x) do { if (!(x)) exit(1); } while (0)
#define OK() do { puts("ok"); return 0; } while (0)
int main(void){int n=13;int a[n];for(int i=0;i<n;i++)a[i]=i+1;typeof(a)*p=&a;int sum=({int s=0;for(int i=0;i<n;i++)s+=(*p)[i];s;});CK(sum==n*(n+1)/2);OK();}
