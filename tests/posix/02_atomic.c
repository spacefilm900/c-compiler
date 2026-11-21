/* Concurrent atomic increments (C11 7.17). Linux fixture: `pthread_t` is
   `unsigned long` in glibc. The POSIX thread functions are declared here
   rather than by including <pthread.h>, because glibc's header uses a GNU
   `aligned` typedef whose size is not a multiple of its alignment, which
   this compiler does not accept. The macOS fixture is
   tests/posix/darwin/02_atomic.c. The native harness links with
   `-pthread`; the Windows Wine stand-in does not run this directory. */

typedef unsigned long pthread_t;

int pthread_create(pthread_t *, const void *, void *(*)(void *), void *);
int pthread_join(pthread_t, void **);
int printf(const char *, ...);

_Atomic int counter;

static void *worker(void *arg)
{
    int n = *(int *)arg;
    int i;
    for (i = 0; i < n; i++)
        counter++;
    return 0;
}

int main(void)
{
    pthread_t t1, t2;
    int a = 10000, b = 10000;
    void *r1, *r2;
    counter = 0;
    if (pthread_create(&t1, 0, worker, &a) != 0)
        return 1;
    if (pthread_create(&t2, 0, worker, &b) != 0)
        return 2;
    if (pthread_join(t1, &r1) != 0 || pthread_join(t2, &r2) != 0)
        return 3;
    printf("%d\n", counter);
    return counter == 20000 ? 0 : 4;
}
