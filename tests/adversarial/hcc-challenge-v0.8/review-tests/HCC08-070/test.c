/* HCC08-070: Atomic qualifier on function typedef
 * 6.7.3p3. Constraint violation: diagnostic required; rejection is challenge policy. */
typedef int F(void); _Atomic F f;
