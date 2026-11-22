/* HCC08-018: Over-aligned argument (16 bytes)
 * 6.7.5; 6.5.2.2; 6.9.1; 7.16 (variadic); target ABI. Separate compilation; verify values across actual ABI boundary. */
struct R {char tag; _Alignas(16) int v; char end;};
int transfer(int,double,struct R,int,struct R);
int transfer(int a,double b,struct R r,int n,struct R q){return a+(int)b+r.tag+r.v+r.end+n+q.v;}
