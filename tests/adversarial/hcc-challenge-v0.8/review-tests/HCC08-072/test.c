/* HCC08-072: Alignment on register object
 * 6.7.5p2. Constraint violation: diagnostic required; rejection is challenge policy. */
int main(void){register _Alignas(16) int x=0;return x;}
