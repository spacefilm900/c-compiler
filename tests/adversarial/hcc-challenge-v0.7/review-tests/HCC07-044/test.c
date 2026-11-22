/* HCC07-044: Atomic type specifier rejects qualified type
 * Basis: 6.7.2.4p3
 * Type name in _Atomic(type-name) may not be qualified.
 * Expected: diagnostic; mode: c11.
 */
_Atomic(const int) a;
