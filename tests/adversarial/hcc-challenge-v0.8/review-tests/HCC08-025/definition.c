/* HCC08-025: Over-aligned return (64 bytes)
 * 6.7.5; 6.5.2.2; 6.9.1; 7.16 (variadic); target ABI. Separate compilation; verify values across actual ABI boundary. */
struct R {char tag; _Alignas(64) int v; char end;};
struct R transfer(struct R, int);
struct R transfer(struct R r,int n){r.v+=n;r.end++;return r;}
