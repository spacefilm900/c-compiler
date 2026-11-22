/* HCC07-014: Mixed TLS declarations
 * Basis: 6.7.1p3
 * Every declaration of a TLS object must contain _Thread_local.
 * Expected: diagnostic; mode: c11.
 */
extern _Thread_local int x; extern int x;
