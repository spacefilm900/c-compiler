/* Trigraphs in string literals only, so the program is valid both ways:
   -std=c11 replaces them (C11 5.2.1.1) and -std=gnu11 leaves the
   characters as written, as GCC does unless -trigraphs is given (GCC
   manual, "Initial processing"). Each mode must match Clang's. */
int printf(const char *, ...);

int main(void)
{
    printf("%s\n", "??=??(??)??<??>??!??'??-");
    printf("%s|%s\n", "??/??/", "???=");
    printf("%d\n", (int)sizeof "??=");
    return 0;
}
