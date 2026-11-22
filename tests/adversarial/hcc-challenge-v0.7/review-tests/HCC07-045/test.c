/* HCC07-045: Atomic type specifier rejects array
 * Basis: 6.7.2.4p3
 * Type name may not specify array type.
 * Expected: diagnostic; mode: c11.
 */
typedef int A[2]; _Atomic(A) a;
