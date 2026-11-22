/* HCC07-007: Aligned typedef constraint
 * Basis: 6.7.5p2
 * Alignment specifier on typedef is forbidden.
 * Expected: diagnostic; mode: c11.
 */
typedef _Alignas(16) int T;
