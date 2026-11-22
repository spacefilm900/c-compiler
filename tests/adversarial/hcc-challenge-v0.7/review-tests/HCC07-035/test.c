/* HCC07-035: VLA member constraint
 * Basis: 6.7.2.1p9
 * Record members cannot have variably modified type.
 * Expected: diagnostic; mode: c11.
 */
void f(int n) { struct S { int a[n]; }; }
