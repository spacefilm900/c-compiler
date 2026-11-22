/* HCC08-067: GNU computed goto exits inner ordinary scope
 * GCC C extensions; N1570 6.5, 6.7, 6.8 as applicable. Documented GNU feature combined with ordinary C semantics. */
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#define CHECK(x) do { if (!(x)) return __LINE__; } while(0)

int main(void){
int k=0;void *labels[]={&&one,&&two};goto *labels[k];one:{int v=7;k=v;}goto *labels[1];two:CHECK(k==7);
puts("ok");return 0;
}
