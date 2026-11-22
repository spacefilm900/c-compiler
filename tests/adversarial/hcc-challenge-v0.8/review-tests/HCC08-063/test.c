/* HCC08-063: GNU omitted conditional with pointer side effects
 * GCC C extensions; N1570 6.5, 6.7, 6.8 as applicable. Documented GNU feature combined with ordinary C semantics. */
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#define CHECK(x) do { if (!(x)) return __LINE__; } while(0)

int main(void){
int a[]={6,9};int *p=a;int *q=p++ ?: a+1;CHECK(q==a&&p==a+1);
puts("ok");return 0;
}
