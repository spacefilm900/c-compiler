/* C11 6.4.2.1, 6.4.3, Annex D: universal character names and UTF-8
   characters in identifiers. A UCN and the UTF-8 character spell the same
   identifier (`g\u00e9n\u00e9ral` is `général`), in every kind of name.
   Macro names, `##` and `#` are covered by the preprocessor unit tests. */
int printf(const char *, ...);
int g\u00e9n\u00e9ral = 42;
int compte_\u00e9tranger(int n) { return n + 3; }
int \u03b1\u03b2 = 3;
static int stat\u00efque = 4;
typedef struct \u00e9tat { int ch\u00e2mp; struct \u00e9tat *suivant; } \u00e9tat;
enum couleur { ROUG\u00c9 = 5, VERT\u00c9 };
static int carr\u00e9(int x\u00b2) { return x\u00b2 * x\u00b2; }
int (*pointeur_\u00e9)(int) = carr\u00e9;
int \U0001F600smile(void) { return 1; }
int extra(void);
int compte(int n) {
    int total\u00e9 = 0;
    static int appels\u00e9;
    appels\u00e9++;
    for (int \u00ef = 0; \u00ef < n; \u00ef++) total\u00e9 += \u00ef;
    if (total\u00e9 > 100) goto fin\u00e9;
    total\u00e9 += 1;
fin\u00e9:
    return total\u00e9 + appels\u00e9;
}
int main(void) {
    état e = { 2, 0 };
    int yé = 9;
    printf("%d %d %d %d %d %d\n", général, compte_étranger(2), αβ, statïque, e.ch\u00e2mp, yé);
    return extra();
}
int extra(void) {
    printf("%d %d %d %d %s %d\n", carré(3), pointeur_é(4), ROUGÉ + VERTÉ, compte(5), "\u00e9t\u00e9", \U0001F600smile());
    return 0;
}
