/* HCC07-019: Extraneous K&R parameter declaration
 * Basis: 6.9.1p6
 * Declaration names must occur in identifier list.
 * Expected: diagnostic; mode: c11.
 */
int f(x) int x; int y; { return x; }
