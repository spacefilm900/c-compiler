/* HCC08-024: Over-aligned callback (32 bytes)
 * 6.7.5; 6.5.2.2; 6.9.1; 7.16 (variadic); target ABI. Separate compilation; verify values across actual ABI boundary. */
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#define CHECK(x) do { if (!(x)) return __LINE__; } while(0)
struct R {char tag; _Alignas(32) int v; char end;};
typedef struct R (*F)(struct R,int); struct R transfer(F,struct R);
static struct R callback(struct R r,int k){r.v+=k;r.end++;return r;}

int main(void){
struct R x={3,517,9}; struct R y=transfer(callback,x); CHECK(y.tag==3 && y.v==527 && y.end==11);
puts("ok");return 0;
}
