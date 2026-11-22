/* HCC07-032: Named goto enters scope of VM typedef
 * Basis: 6.8.6.1p1
 * Jump into scope of a variably modified identifier is forbidden even if it is a typedef.
 * Expected: diagnostic; mode: c11.
 */
int f(int n) { goto L; typedef int A[n]; L: return 0; }
