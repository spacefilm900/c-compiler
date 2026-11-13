/* GNU C: zero-length arrays (GCC manual, "Arrays of Length Zero"). A
   trailing zero-length member gives a variable-length object; the
   member adds no size. */

int printf(const char *, ...);
void *malloc(unsigned long);
void free(void *);

struct line {
    int length;
    char contents[0];
};

struct header {
    short tag;
    long values[0];
};

struct mid {
    char c;
    int none[0];
    char d;
};

int empty_global[0];

int main(void)
{
    struct line *l = malloc(sizeof(struct line) + 6);
    l->length = 5;
    for (int i = 0; i < 5; i++)
        l->contents[i] = 'a' + i;
    l->contents[5] = 0;
    printf("%zu %zu %zu %s\n", sizeof(struct line), sizeof(struct header), sizeof(struct mid),
           l->contents);
    printf("%zu %zu\n", __builtin_offsetof(struct header, values),
           __builtin_offsetof(struct mid, d));
    printf("%zu\n", sizeof(empty_global));
    free(l);
    return 0;
}
