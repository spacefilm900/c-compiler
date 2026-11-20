/* Concurrent atomic increments (C11 7.17). macOS fixture: `pthread_t` is
   a pointer to `struct _opaque_pthread_t`. The functions are declared
   here so this file has no preprocessor directives. The Linux fixture is
   tests/posix/02_atomic.c. The native harness links with `-pthread`. */

typedef struct _opaque_pthread_t *pthread_t;

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
