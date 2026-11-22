/* HCC07-046: Atomic bit-field constraint
 * Basis: 6.7.3p3
 * Atomic qualifier must not be used with bit-field.
 * Expected: diagnostic; mode: c11.
 */
struct S { _Atomic unsigned a:3; };
