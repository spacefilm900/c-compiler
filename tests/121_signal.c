void exit(int);
typedef void (*handler)(int);
/* Two names for one symbol: the module declares it once. */
handler first_signal(int, handler) __asm__("__sysv_signal");
handler again_signal(int, handler) __asm__("__sysv_signal");
static void h(int s) { (void)s; exit(0); }
int main(void) {
    first_signal(8, h);
    again_signal(8, h);
    return 0;
}
