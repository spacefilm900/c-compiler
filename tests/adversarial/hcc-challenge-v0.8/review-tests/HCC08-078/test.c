/* HCC08-078: Old-style parameter adjusted array
 * 6.5.2.2; 6.7.6.3; 6.9.1; 7.25. Check documented promoted parameter/real generic dispatch. */
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#define CHECK(x) do { if (!(x)) return __LINE__; } while(0)
#include <tgmath.h>
int f(a,n) int a[]; int n; {int s=0;for(int i=0;i<n;i++)s+=a[i];return s;}
int main(void){
int a[3]={1,2,3};CHECK(f(a,3)==6);
puts("ok");return 0;
}
