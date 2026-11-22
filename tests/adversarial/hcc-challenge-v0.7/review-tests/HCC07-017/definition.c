/* HCC07-017: K&R caller and prototype definition separate units
 * Basis: 6.7.6.3p14,p15; 6.5.2.2
 * Nonprototype call promotes float and narrow integer; separate definition uses promoted types.
 * Expected: exit 0; stdout: ok; mode: c11.
 */
int f(int a, double b) { return a+(int)(4*b); }
