/* HCC07-008: Aligned bit-field constraint
 * Basis: 6.7.5p2
 * Alignment specifier cannot apply to bit-field.
 * Expected: diagnostic; mode: c11.
 */
struct S { _Alignas(16) unsigned x:3; };
