/* HCC07-009: Aligned parameter constraint
 * Basis: 6.7.5p2
 * Parameter may not bear alignment specifier.
 * Expected: diagnostic; mode: c11.
 */
int f(_Alignas(16) int x);
