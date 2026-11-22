/* HCC08-076: VLA named goto into nested block
 * 6.8.6.1p1. Constraint violation: diagnostic required; rejection is challenge policy. */
int f(int n){goto in;{int a[n];in:return sizeof a;}}
