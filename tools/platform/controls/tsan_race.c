/* Known-positive control for the ThreadSanitizer run: two threads
   increment a plain int with no synchronisation (a data race). The
   sanitizer must report it. Never run in the gating suites. */
#include <stdio.h>
#include <threads.h>

static int counter;

static int work(void *arg) {
    (void)arg;
    for (int i = 0; i < 100000; i++)
        counter++;
    return 0;
}

int main(void) {
    thrd_t a, b;
    if (thrd_create(&a, work, NULL) != thrd_success || thrd_create(&b, work, NULL) != thrd_success)
        return 2;
    thrd_join(a, NULL);
    thrd_join(b, NULL);
    printf("%d\n", counter > 0);
    return 0;
}
