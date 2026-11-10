#include <uchar.h>

int main(void) {
    char16_t c = u'A';
    char32_t d = U'B';
    char bytes[] = u8"hi";
    char16_t us[] = u"Ab";
    char32_t wide[] = U"Ab";
    char16_t cat[] = u"A" "b";
    char both[] = "h" u8"i";
    char32_t adopted[] = "A" U"b";
    if (sizeof(c) != 2 || c != u'A' || u'\xff' != 255) {
        return 1;
    }
    if (sizeof(d) != 4 || d != U'B' || U'\x100' != 256) {
        return 2;
    }
    if (sizeof(bytes) != 3 || bytes[0] != 'h' || bytes[1] != 'i' || bytes[2] != 0) {
        return 3;
    }
    if (sizeof(us) != 6 || us[0] != u'A' || us[1] != u'b' || us[2] != 0) {
        return 4;
    }
    if (sizeof(wide) != 12 || wide[0] != U'A' || wide[1] != U'b' || wide[2] != 0) {
        return 5;
    }
    if (sizeof(cat) != 6 || cat[0] != 'A' || cat[1] != 'b' || cat[2] != 0) {
        return 6;
    }
    if (sizeof(both) != 3 || both[0] != 'h' || both[1] != 'i') {
        return 7;
    }
    if (sizeof(adopted) != 12 || adopted[0] != 'A' || adopted[1] != 'b') {
        return 8;
    }
    if (sizeof(u"") != 2 || sizeof(U"") != 4 || sizeof(u8"") != 1) {
        return 9;
    }
    return 0;
}
