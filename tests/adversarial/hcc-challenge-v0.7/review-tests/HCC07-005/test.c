/* HCC07-005: Alignment cannot weaken natural alignment
 * Basis: 6.7.5p4
 * Combined alignment below that of double violates a constraint.
 * Expected: diagnostic; mode: c11.
 */
_Alignas(1) double x;
