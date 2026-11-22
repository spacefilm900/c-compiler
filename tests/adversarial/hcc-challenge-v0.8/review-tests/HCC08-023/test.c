/* HCC08-023: Over-aligned variadic (32 bytes)
 * 6.7.5; 6.5.2.2; 6.9.1; 7.16 (variadic); target ABI. Separate compilation; verify values across actual ABI boundary. */
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#define CHECK(x) do { if (!(x)) return __LINE__; } while(0)
struct R {char tag; _Alignas(32) int v; char end;};
int transfer(int,...);

int main(void){
struct R x={3,517,9}; struct R y={1,123,2}; CHECK(transfer(4,8,x,7.0,y)==664);
puts("ok");return 0;
}
