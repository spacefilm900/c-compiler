/* HCC08-017: Over-aligned return (16 bytes)
 * 6.7.5; 6.5.2.2; 6.9.1; 7.16 (variadic); target ABI. Separate compilation; verify values across actual ABI boundary. */
struct R {char tag; _Alignas(16) int v; char end;};
struct R transfer(struct R, int);
struct R transfer(struct R r,int n){r.v+=n;r.end++;return r;}
