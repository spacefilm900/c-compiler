/* GNU `cond ?: else` with floating operands in static initializers (GCC
   manual, "Conditionals with Omitted Operands"): the condition is folded
   once and, when nonzero, is the value, converted to the result type. */
int printf(const char *, ...);
double x = 1.5 ?: 2.0;
double y = 0.0 ?: 2.5;
float z = 0 ?: 3.25f;
double w = 2 ?: 9.5;
int i = 0.0 ?: 7;
long double ld = 0.5L ?: 1.0L;
int main(void) { printf("%g %g %g %g %d %Lg\n", x, y, z, w, i, ld); return 0; }
