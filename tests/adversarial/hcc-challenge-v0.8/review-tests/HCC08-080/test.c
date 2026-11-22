/* HCC08-080: Type generic real math on VLA elements
 * 6.5.2.2; 6.7.6.3; 6.9.1; 7.25. Check documented promoted parameter/real generic dispatch. */
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#define CHECK(x) do { if (!(x)) return __LINE__; } while(0)
#include <tgmath.h>

int main(void){
int n=3;float a[n];a[1]=16.0f;int k=1;CHECK(_Generic(sqrt(a[k]),float:1,default:0));CHECK(sqrt(a[k++])==4.0f&&k==2);
puts("ok");return 0;
}
