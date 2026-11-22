/* HCC08-071: Atomic type specifier rejects already atomic type
 * 6.7.2.4p3. Constraint violation: diagnostic required; rejection is challenge policy. */
typedef _Atomic int A; _Atomic(A) x;
