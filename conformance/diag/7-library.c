/* N1570 7 library: constraints on macros and headers */
//== 7.2 static_assert from <assert.h> fails
#include <assert.h>
static_assert(0, "fails");
//== 7.17.1 an atomic type in a non-lock-free size is diagnosed or supported; a failing _Static_assert on ATOMIC_INT_LOCK_FREE
#include <stdatomic.h>
_Static_assert(ATOMIC_INT_LOCK_FREE == 0, "int is lock-free");
//== 7.19 [policy] offsetof a bit-field
#include <stddef.h>
struct s { int b : 3; }; unsigned long o = offsetof(struct s, b);
//== 7.16.1.4 [policy] va_start in a function without ...
#include <stdarg.h>
void h(int n) { va_list ap; va_start(ap, n); va_end(ap); }
