/* HCC07-061: Aligned compound literal and record ABI across translation units
 * Basis: 6.7.5; 6.5.2.5; 6.5.2.2; 6.9.1
 * An aligned member changes record layout; passing and returning this type must preserve fields and alignment in mixed object files.
 * Expected: exit 0; stdout: ok; mode: c11.
 */
struct R { _Alignas(16) double value; int tag; };
struct R f(struct R r) { r.value*=2; r.tag++; return r; }
