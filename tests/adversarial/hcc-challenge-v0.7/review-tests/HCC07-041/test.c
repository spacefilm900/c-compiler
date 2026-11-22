/* HCC07-041: Atomic aggregate load store and exchange
 * Basis: 6.5.16.1; 7.17.7.2,p3
 * Eight-byte, padding-free record is within supported atomic size; whole record exchanged.
 * Expected: exit 0; stdout: ok; mode: c11.
 */
#include <stdio.h>
#define CHECK(x) do { if (!(x)) return 1; } while (0)
#include <stdatomic.h>
struct P { int a,b; };
int main(void) {
struct P x={3,7}, y={9,2}; _Atomic(struct P) a=x;
    struct P old=atomic_exchange(&a,y); struct P now=atomic_load(&a); CHECK(old.a==3 && old.b==7 && now.a==9 && now.b==2);
    puts("ok"); return 0;
}
