/* Extended identifiers as external symbols: defined here, used by
   10_ucn_names_use.c. The two files spell the names differently (UCN here,
   UTF-8 there); the symbols must still match. */
int g\u00e9n\u00e9ral = 42;
int compte_\u00e9tranger(int n) { return n * 2; }
double \u03c0\u00b2(void) { return 9.8696; }
struct \u00e9tat { int \u00e2ge; char nom[8]; };
struct \u00e9tat lire_\u00e9tat(int a) { struct \u00e9tat e = { a, "ok" }; return e; }
int \U0001F600(void) { return 1; }
