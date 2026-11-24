/* The _Pragma operator (C11 6.10.9) is destringized and carried out as a
   #pragma line, in the source and in a macro's replacement. Pragmas this
   compiler does not act on are ignored, as on a #pragma line; the operator
   leaves no tokens behind, so the declarations around it are intact. */
int printf(const char *, ...);

_Pragma("STDC FP_CONTRACT OFF")
_Pragma(L"STDC FENV_ACCESS OFF") int a = 1;
int b = 2 _Pragma("GCC diagnostic ignored \"-Wunused\"");

int main(void)
{
    _Pragma("STDC CX_LIMITED_RANGE DEFAULT")
    int c = a + b;
    printf("%d\n", c);
    return c - 3;
}
