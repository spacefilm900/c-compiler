/* File-scope objects with internal linkage may use the names of C
   library functions when no header declares them (C11 7.1.3p1 reserves
   nan, memcpy, and the rest at file scope only for a translation unit
   that includes the associated header). The compiler itself calls nan()
   for a __builtin_nan tag that is not a constant, and LLVM may call
   memcpy, memmove, and memset, so the objects must not take over those
   symbols. f() is not called, so the reference compiler, which lets the
   call reach the int object, gives the same result. */

static int nan = 7;
static float nanf = 1.5f;
static int memcpy = 2;
static int memmove = 3;
static int memset = 4;

double f(void) {
    return __builtin_nan("1xyz") + __builtin_nanf("x");
}

int main(void) {
    return nan + (int)(nanf * 2.0f) + memcpy + memmove + memset - 19;
}
