/* HCC07-006: Non-power-of-two alignment
 * Basis: 6.7.5p3; 6.2.8p4
 * Three is not a valid alignment.
 * Expected: diagnostic; mode: c11.
 */
_Alignas(3) int x;
