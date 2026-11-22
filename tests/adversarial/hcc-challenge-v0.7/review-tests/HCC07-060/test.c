/* HCC07-060: stdalign header and type-name alignment
 * Basis: 7.15; 6.7.5p5
 * Header aliases and feature macros are provided; type-name form has the same alignment requirement as alignof.
 * Expected: exit 0; stdout: ok; mode: c11.
 */
#include <stdio.h>
#define CHECK(x) do { if (!(x)) return 1; } while (0)
#include <stdalign.h>
#include <stdint.h>

int main(void) {
alignas(long double) char a=7; CHECK(__alignas_is_defined==1 && __alignof_is_defined==1); CHECK((uintptr_t)&a%alignof(long double)==0 && a==7);
    puts("ok"); return 0;
}
