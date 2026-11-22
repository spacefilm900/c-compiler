/* HCC08-017: Over-aligned return (16 bytes)
 * 6.7.5; 6.5.2.2; 6.9.1; 7.16 (variadic); target ABI. Separate compilation; verify values across actual ABI boundary. */
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#define CHECK(x) do { if (!(x)) return __LINE__; } while(0)
struct R {char tag; _Alignas(16) int v; char end;};
struct R transfer(struct R, int);

int main(void){
struct R x={3,517,9}; struct R y=transfer(x,21); CHECK(y.tag==3 && y.v==538 && y.end==10 && x.v==517);
puts("ok");return 0;
}
