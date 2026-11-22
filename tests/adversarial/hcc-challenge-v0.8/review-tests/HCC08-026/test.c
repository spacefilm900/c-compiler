/* HCC08-026: Over-aligned argument (64 bytes)
 * 6.7.5; 6.5.2.2; 6.9.1; 7.16 (variadic); target ABI. Separate compilation; verify values across actual ABI boundary. */
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#define CHECK(x) do { if (!(x)) return __LINE__; } while(0)
struct R {char tag; _Alignas(64) int v; char end;};
int transfer(int,double,struct R,int,struct R);

int main(void){
struct R x={3,517,9}; struct R y={1,123,2}; CHECK(transfer(5,6.0,x,11,y)==674);
puts("ok");return 0;
}
