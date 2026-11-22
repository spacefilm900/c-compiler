/* Regression control (callee, built by this compiler): a two-eightbyte
   struct parameter read after a local whose address escapes. Under
   AddressSanitizer at -O0 an earlier hcc returned 0 here; see
   store_param in src/irgen/call.rs. */
struct s { long a; void *b; };
void sink(long *p);

long get_a(struct s x) {
    long local = 0;
    sink(&local);
    return x.a + local;
}
