/* _Thread_local with multiple threads: each thread has a distinct object
   (C11 6.2.4p4). Linux fixture: `pthread_t` is `unsigned long` in glibc.
   The POSIX thread functions are declared here rather than by including
   <pthread.h>, because glibc's header uses a GNU `aligned` typedef whose
   size is not a multiple of its alignment, which this compiler does not
   accept. The macOS fixture is tests/posix/darwin/01_tls_threads.c. The
   native harness links with `-pthread`; the Windows Wine stand-in does
   not run this directory. */

typedef unsigned long pthread_t;

int pthread_create(pthread_t *, const void *, void *(*)(void *), void *);
int pthread_join(pthread_t, void **);
int printf(const char *, ...);

_Thread_local int slot = 1;

static void *worker(void *arg) {
    int id = *(int *)arg;
    slot = id * 10;
    slot = slot + 1;
    return (void *)(long)slot;
}

int main(void) {
    pthread_t t1, t2;
    int a = 2, b = 3;
    void *r1, *r2;
    slot = 7;
    if (pthread_create(&t1, 0, worker, &a) != 0)
        return 1;
    if (pthread_create(&t2, 0, worker, &b) != 0)
        return 2;
    if (pthread_join(t1, &r1) != 0 || pthread_join(t2, &r2) != 0)
        return 3;
    printf("%d %d %d\n", slot, (int)(long)r1, (int)(long)r2);
    return (slot == 7 && (int)(long)r1 == 21 && (int)(long)r2 == 31) ? 0 : 4;
}
