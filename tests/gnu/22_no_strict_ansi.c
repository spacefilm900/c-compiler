/* GNU C: `__STRICT_ANSI__` is defined only for a strict -std (GCC manual,
   "Common Predefined Macros"), so under -std=gnu11 a C library shows its
   POSIX and BSD declarations. hcc defined it in every mode, and glibc's
   <stdio.h> then hid `fileno`, which zlib's minigzip.c uses. Strict C11
   still defines it, and this file is rejected there. */
#ifdef __STRICT_ANSI__
#error "__STRICT_ANSI__ is defined under -std=gnu11"
#endif
int printf(const char *, ...);

int main(void) {
    printf("%d\n", __STDC_VERSION__ >= 201112L);
    return 0;
}
