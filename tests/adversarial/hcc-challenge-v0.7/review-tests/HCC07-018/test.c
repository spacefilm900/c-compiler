/* HCC07-018: Prototype incompatible with promoted float
 * Basis: 6.7.6.3p15
 * Prior float parameter is incompatible with old-style float parameter promoted to double.
 * Expected: diagnostic; mode: c11.
 */
int f(float); int f(x) float x; { return 0; }
