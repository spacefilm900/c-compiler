/* HCC07-033: Switch bypasses VM pointer declaration
 * Basis: 6.8.4.2p2
 * A switch label in scope of VM identifier requires switch statement wholly in that scope.
 * Expected: diagnostic; mode: c11.
 */
int f(int n,int x) { switch(x) { int (*p)[n]; case 1: return 1; } return 0; }
