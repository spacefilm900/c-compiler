/* HCC08-020: Over-aligned callback (16 bytes)
 * 6.7.5; 6.5.2.2; 6.9.1; 7.16 (variadic); target ABI. Separate compilation; verify values across actual ABI boundary. */
struct R {char tag; _Alignas(16) int v; char end;};
typedef struct R (*F)(struct R,int); struct R transfer(F,struct R);
struct R transfer(F f,struct R r){return f(f(r,3),7);}
