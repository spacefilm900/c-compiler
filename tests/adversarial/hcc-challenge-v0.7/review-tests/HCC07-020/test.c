/* HCC07-020: Duplicate K&R parameter declaration
 * Basis: 6.9.1p6
 * Each identifier in list is declared only once.
 * Expected: diagnostic; mode: c11.
 */
int f(x) int x; int x; { return x; }
