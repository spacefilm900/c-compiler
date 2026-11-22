/* HCC08-074: Thread-local member
 * 6.7.1; 6.7.2.1. Constraint violation: diagnostic required; rejection is challenge policy. */
struct S{_Thread_local int x;};
