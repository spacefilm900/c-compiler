/* HCC07-034: Variably modified generic association
 * Basis: 6.5.1.1p2
 * Association type may not be variably modified, even when unselected.
 * Expected: diagnostic; mode: c11.
 */
int f(int n) { return _Generic(1,int:1,int (*)[n]:2); }
