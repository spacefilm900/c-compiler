#include <stddef.h>
#include <stdio.h>

int main(void) {
    wchar_t zero = L'\0';
    wchar_t letter = L'A';
    wchar_t nl = L'\n';
    wchar_t high = L'\xFF';
    wchar_t text[] = L"Ab";
    wchar_t one = L'\0';
    printf("%d %d %d %d %d %d %zu %d\n", zero, letter, nl, high, text[0], text[1], sizeof text,
           one);
    if (zero != 0 || letter != 'A' || text[2] != 0) {
        return 1;
    }
    return 0;
}
