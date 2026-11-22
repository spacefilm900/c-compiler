/* HCC07-013: TLS forbidden on function
 * Basis: 6.7.1p2
 * Only objects may be thread-local.
 * Expected: diagnostic; mode: c11.
 */
_Thread_local int f(void);
