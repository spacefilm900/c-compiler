/* HCC07-021: Type-generic math with side effects and qualifiers
 * Basis: 7.25p3; 6.5.1.1
 * Type selection does not evaluate arguments; call consumes each once; qualifiers do not change real function domain.
 * Expected: exit 0; stdout: ok; mode: c11.
 */
#include <stdio.h>
#define CHECK(x) do { if (!(x)) return 1; } while (0)
#include <tgmath.h>

int main(void) {
volatile float a=9.0f; int n=0; float x[2]={4.0f,9.0f};
    CHECK(_Generic(sqrt(a),float:1,default:0));
    float y=sqrt(x[n++]); CHECK(n==1 && y==2.0f);
    CHECK(_Generic(fmax(a,2),double:1,default:0));
    puts("ok"); return 0;
}
