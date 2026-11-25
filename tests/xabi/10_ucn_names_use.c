int printf(const char *, ...);
extern int général;
int compte_étranger(int);
double π²(void);
struct état { int âge; char nom[8]; };
struct état lire_état(int);
int 😀(void);
int main(void) {
    struct état e = lire_état(7);
    printf("%d %d %.4f %d %s %d\n", général, compte_étranger(5), π²(), e.âge, e.nom, 😀());
    return 0;
}
