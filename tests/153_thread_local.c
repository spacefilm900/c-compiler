/* _Thread_local (C11 6.7.1, 6.2.4p4) in a single thread: distinct objects,
   static duration of the value, and block-scope static thread-local. */
int printf(const char *, ...);

_Thread_local int g;
_Thread_local static int s = 7;
extern _Thread_local int g;

int bump(void) {
    static _Thread_local int loc = 3;
    loc = loc + 1;
    return loc;
}

int main(void) {
    g = 10;
    s = s + 1;
    printf("%d %d %d %d\n", g, s, bump(), bump());
    return 0;
}
