/* Result types of the GCC builtins this compiler supports, seen from
   constant expressions the parser folds: __builtin_signbit, signbitf
   and signbitl yield int; __builtin_memcpy, memmove and memset yield
   void * (GCC manual, Other Builtins and Library Builtins). The
   operand of sizeof and the controlling expression of _Generic are not
   evaluated (C11 6.5.3.4p2, 6.5.1.1p3), so these calls never run. */

static char buf[8];
static double d;

enum {
    SIGNBIT_SIZE = sizeof(__builtin_signbit(1.0)),
    SIGNBITF_SIZE = sizeof(__builtin_signbitf(1.0f)),
    SIGNBITL_SIZE = sizeof(__builtin_signbitl(1.0L)),
    MEMCPY_SIZE = sizeof(__builtin_memcpy(buf, buf, 1)),
    SIGNBIT_IS_INT = _Generic(__builtin_signbit(-2.0), int: 1, default: 0),
    MEMSET_IS_VOIDP = _Generic(__builtin_memset(buf, 0, 1), void *: 1, default: 0),
    NESTED = sizeof(__builtin_isgreater(__builtin_signbit(1.0), 2.0)),
    CMP = sizeof(__builtin_memmove(buf, &d, 1) == &d)
};

_Static_assert(sizeof(__builtin_signbit(1.0)) == sizeof(int), "signbit yields int");
_Static_assert(sizeof(__builtin_memset(buf, 0, 1)) == sizeof(void *), "memset yields void *");

static int sizes[sizeof(__builtin_signbitl(2.0L)) + sizeof(__builtin_memcpy(buf, buf, 0))];

int main(void) {
    if (SIGNBIT_SIZE != sizeof(int) || SIGNBITF_SIZE != sizeof(int) || SIGNBITL_SIZE != sizeof(int))
        return 1;
    if (MEMCPY_SIZE != sizeof(void *))
        return 2;
    if (!SIGNBIT_IS_INT || !MEMSET_IS_VOIDP)
        return 3;
    if (NESTED != sizeof(int) || CMP != sizeof(int))
        return 4;
    if (sizeof sizes != (sizeof(int) + sizeof(void *)) * sizeof(int))
        return 5;
    return buf[0] + (int)d;
}
