/* HCC07-037: VLA static storage constraint
 * Basis: 6.7.6.2p2
 * Static storage duration object cannot have variable length array type.
 * Expected: diagnostic; mode: c11.
 */
void f(int n) { static int a[n]; }
