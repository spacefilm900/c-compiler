/* HCC08-023: Over-aligned variadic (32 bytes)
 * 6.7.5; 6.5.2.2; 6.9.1; 7.16 (variadic); target ABI. Separate compilation; verify values across actual ABI boundary. */
struct R {char tag; _Alignas(32) int v; char end;};
int transfer(int,...);
#include <stdarg.h>
int transfer(int n,...){va_list ap;va_start(ap,n);int pre=va_arg(ap,int);struct R r=va_arg(ap,struct R);double d=va_arg(ap,double);struct R q=va_arg(ap,struct R);va_end(ap);return n+pre+r.v+r.tag+(int)d+q.v+q.end;}
