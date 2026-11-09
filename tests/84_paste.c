#define STR(x) #x
#define XSTR(x) STR(x)
#define N 41
#define CAT(a, b) a##b

int CAT(ma, in)(void) {
    const char *s = XSTR(N);
    const char *t = STR(a + b);
    if (s[0] != '4' || s[1] != '1' || s[2] != 0) {
        return 1;
    }
    if (t[0] != 'a' || t[1] != ' ' || t[2] != '+' || t[3] != ' ' || t[4] != 'b' || t[5] != 0) {
        return 2;
    }
    return 0;
}
