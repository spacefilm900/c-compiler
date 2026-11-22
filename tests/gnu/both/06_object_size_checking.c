/* Object size checking builtins (GCC manual, "Object Size Checking").
   System headers call these: the macOS SDK's <stdio.h> spells vsnprintf
   as __builtin___vsnprintf_chk. Only the memory builtins are run here,
   with the constant object size (size_t)-1, because Clang then calls the
   plain function at every optimization level; for the string and
   formatted-output builtins Clang calls the C library's __*_chk entry
   points, which Windows does not have. Those are covered by the IR tests
   in src/irgen/precise_tests.rs. */

int printf(const char *, ...);
int puts(const char *);

static int calls;

static char *id(char *p)
{
    return p;
}

static char *counted(char *p)
{
    calls++;
    return p;
}

int main(void)
{
    char storage[16];
    char *d = id(storage);

    /* The operand is never evaluated. Unknown sizes are (size_t)-1 for
       types 0 and 1, and 0 for types 2 and 3. */
    printf("%d %d %d %d\n",
           __builtin_object_size(counted(d), 0) == (unsigned long)-1,
           __builtin_object_size(counted(d), 1) == (unsigned long)-1,
           __builtin_object_size(counted(d), 2) == 0,
           __builtin_dynamic_object_size(counted(d), 3) == 0);
    printf("calls %d %d\n", calls, (int)sizeof(__builtin_object_size(d, 0)));

    __builtin___memset_chk(d, 0, sizeof storage, (unsigned long)-1);
    __builtin___memcpy_chk(d, "abcdef", 6, (unsigned long)-1);
    puts(d);
    char *r = __builtin___memmove_chk(d + 1, d, 6, (unsigned long)-1);
    printf("%s %d\n", d, (int)(r - d));
    return 0;
}
