/* HCC07-036: VLA initializer constraint
 * Basis: 6.7.9p3
 * Variable length array objects cannot be initialized.
 * Expected: diagnostic; mode: c11.
 */
void f(int n) { int a[n]={0}; }
