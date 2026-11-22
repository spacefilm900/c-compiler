/* HCC08-040: Atomic flag in nested aligned record
 * 6.5.16.2; 7.17.3; 7.17.7; 7.17.8. Defined operation and exact state checks; no padding comparison. */
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#define CHECK(x) do { if (!(x)) return __LINE__; } while(0)
#include <stdatomic.h>

int main(void){
struct I{char x; _Alignas(32) atomic_flag f;}; struct O{char x;struct I i;}; struct O o={.i={.f=ATOMIC_FLAG_INIT}}; CHECK((uintptr_t)&o.i.f%32==0); CHECK(!atomic_flag_test_and_set(&o.i.f)); CHECK(atomic_flag_test_and_set(&o.i.f)); atomic_flag_clear(&o.i.f); CHECK(!atomic_flag_test_and_set(&o.i.f));
puts("ok");return 0;
}
