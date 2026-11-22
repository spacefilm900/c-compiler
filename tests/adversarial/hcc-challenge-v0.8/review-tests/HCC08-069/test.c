/* HCC08-069: Atomic type specifier rejects const-qualified type
 * 6.7.2.4p3. Constraint violation: diagnostic required; rejection is challenge policy. */
_Atomic(const int) x;
