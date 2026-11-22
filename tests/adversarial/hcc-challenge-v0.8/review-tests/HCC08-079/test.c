/* HCC08-079: Type generic fma mixes integer and float
 * 6.5.2.2; 6.7.6.3; 6.9.1; 7.25. Check documented promoted parameter/real generic dispatch. */
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#define CHECK(x) do { if (!(x)) return __LINE__; } while(0)
#include <tgmath.h>

int main(void){
float a=2.0f;CHECK(_Generic(fma(a,3,4.0f),double:1,default:0));CHECK(fma(a,3,4.0f)==10.0);
puts("ok");return 0;
}
