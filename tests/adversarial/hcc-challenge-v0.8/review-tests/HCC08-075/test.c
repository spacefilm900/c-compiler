/* HCC08-075: VLA file scope via variable bound
 * 6.7.6.2p2. Constraint violation: diagnostic required; rejection is challenge policy. */
int n=3;typedef int A[n];
