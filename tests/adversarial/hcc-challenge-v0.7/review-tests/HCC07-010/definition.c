/* HCC07-010: TLS extern across translation units
 * Basis: 6.7.1p3; 6.2.4p4
 * All declarations carry TLS; each thread has its own instance; thread starts with initializer.
 * Expected: exit 0; stdout: ok; mode: c11.
 */
_Thread_local int g=11;
