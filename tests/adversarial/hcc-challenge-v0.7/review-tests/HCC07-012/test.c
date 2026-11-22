/* HCC07-012: TLS missing at block scope
 * Basis: 6.7.1p3
 * Block scope TLS must be static or extern.
 * Expected: diagnostic; mode: c11.
 */
void f(void) { _Thread_local int x; (void)x; }
