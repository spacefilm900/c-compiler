/* HCC07-022: Non-generic parameters do not select precision
 * Basis: 7.25p3; 7.12.6.4; 7.12.10.3
 * Exponent and pointer parameters do not influence type selection; frexp and remquo return exact values.
 * Expected: exit 0; stdout: ok; mode: c11.
 */
#include <stdio.h>
#define CHECK(x) do { if (!(x)) return 1; } while (0)
#include <tgmath.h>

int main(void) {
int e=0; int q=0;
    CHECK(_Generic(scalbn(1.5f,2),float:1,default:0));
    CHECK(_Generic(frexp(8.0f,&e),float:1,default:0));
    CHECK(frexp(8.0f,&e)==0.5f && e==4);
    CHECK(_Generic(remquo(7.0f,2.0f,&q),float:1,default:0)); CHECK(remquo(7.0f,2.0f,&q)==-1.0f && q>0 && (q&7)==4);
    puts("ok"); return 0;
}
