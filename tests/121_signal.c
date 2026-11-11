#include <signal.h>
#include <stdlib.h>
static void h(int s) { (void)s; exit(0); }
int main(void) { signal(SIGFPE, h); return 0; }
