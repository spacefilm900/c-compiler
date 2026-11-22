/* HCC08-077: K&R float promotion into type generic math
 * 6.5.2.2; 6.7.6.3; 6.9.1; 7.25. Check documented promoted parameter/real generic dispatch. */
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#define CHECK(x) do { if (!(x)) return __LINE__; } while(0)
#include <tgmath.h>
float f(double); float f(x) float x; {return sqrt(x);}
int main(void){
CHECK(f(9.0f)==3.0f);
puts("ok");return 0;
}
