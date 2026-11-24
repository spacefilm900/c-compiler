/* Trigraphs (C11 5.2.1.1): translation phase 1 replaces the nine
   sequences, in code and inside literals, before lines are spliced. Strict
   C11, the harness default, replaces them. tests/gnu/both/08_trigraphs.c
   checks that -std=gnu11 leaves them alone. */
int printf(const char *, ...);

int table??(3??) = ??< 4, 5, 6 ??>;

int main(void)
??<
    int mixed = (table??(0??) ??! 8) ??' 1;
    int flipped = ??-table??(1??);
    const char *text = "a??/"b??/??/c";
    const char *kept = "?? ??? ?a ??x";
    int spliced = 1 + ??/
2;
    printf("%d %d %s %s %d\n", mixed, flipped, text, kept, spliced);
    printf("%s\n", "??=??(??)??<??>??!??'??-");
    printf("%d\n", '??/n' == '\n');
    return table??(2??) - 6;
??>
