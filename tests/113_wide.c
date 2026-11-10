typedef int wchar_t;
int printf(const char *, ...);

int main(void) {
    wchar_t zero = L'\0';
    wchar_t letter = L'A';
    wchar_t nl = L'\n';
    wchar_t high = L'\xFF';
    wchar_t text[] = L"Ab";
    wchar_t one = L'\0';
    wchar_t left[] = L"A" "b";
    wchar_t right[] = "A" L"b";
    printf("%d %d %d %d %d %d %zu %d\n", zero, letter, nl, high, text[0], text[1], sizeof text,
           one);
    if (zero != 0 || letter != 'A' || text[2] != 0) {
        return 1;
    }
    if (left[0] != 'A' || left[1] != 'b' || left[2] != 0 || sizeof left != 12) {
        return 2;
    }
    if (right[0] != 'A' || right[1] != 'b' || right[2] != 0 || sizeof right != 12) {
        return 3;
    }
    return 0;
}
