/* An array of unions with no length, whose elements name different union
   members. Working out the length walks the elements once on their own;
   that walk must not leave anything behind for the real one, or the
   statement expression's label below is checked twice and reported as a
   duplicate. Found by review of the union member-switch fix (PR 32). */

int printf(const char *, ...);

union U { int a; int b; };

int main(void)
{
    union U v[] = {
        {.a = ({ once: 0; 1; })},
        {.b = 2},
    };
    printf("%d %d %d\n", (int)(sizeof v / sizeof v[0]), v[0].a, v[1].b);
    return v[0].a != 1 || v[1].b != 2;
}
