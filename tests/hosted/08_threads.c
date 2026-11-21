/* C11 <threads.h>: call_once, mutexes, condition variables, TSS,
   thread_local, and join. Output is deterministic. */
#include <stdio.h>
#include <threads.h>
#include <time.h>

static once_flag once = ONCE_FLAG_INIT;
static int once_count;
static mtx_t mtx;
static mtx_t timed;
static cnd_t cnd;
static int ready;
static tss_t key;
static thread_local int tls;
static int try_res;

static void once_fn(void) {
    once_count++;
}

static int worker(void *arg) {
    int n = *(int *)arg;
    call_once(&once, once_fn);
    tls = n;
    if (tss_set(key, &tls) != thrd_success) {
        return -1;
    }
    if (mtx_lock(&mtx) != thrd_success) {
        return -1;
    }
    while (!ready) {
        if (cnd_wait(&cnd, &mtx) != thrd_success) {
            mtx_unlock(&mtx);
            return -1;
        }
    }
    {
        int *p = (int *)tss_get(key);
        int v = p ? *p : 0;
        mtx_unlock(&mtx);
        return v;
    }
}

static int tryer(void *arg) {
    (void)arg;
    try_res = mtx_trylock(&mtx);
    if (try_res == thrd_success) {
        mtx_unlock(&mtx);
    }
    return 0;
}

static int timed_tryer(void *arg) {
    struct timespec past;
    (void)arg;
    past.tv_sec = 0;
    past.tv_nsec = 0;
    return mtx_timedlock(&timed, &past);
}

int main(void) {
    int a = 2, b = 3, ra = 0, rb = 0, rt = 0;
    thrd_t ta, tb, tt;
    struct timespec tiny;
    mtx_t rec;
    int rec_ok, timed_ok, sleep_ok, eq, yield_ok;

    if (mtx_init(&mtx, mtx_plain) != thrd_success) {
        return 1;
    }
    if (mtx_init(&timed, mtx_timed) != thrd_success) {
        return 1;
    }
    if (mtx_init(&rec, mtx_plain | mtx_recursive) != thrd_success) {
        return 1;
    }
    if (cnd_init(&cnd) != thrd_success) {
        return 1;
    }
    if (tss_create(&key, 0) != thrd_success) {
        return 1;
    }

    call_once(&once, once_fn);

    if (thrd_create(&ta, worker, &a) != thrd_success) {
        return 1;
    }
    if (thrd_create(&tb, worker, &b) != thrd_success) {
        return 1;
    }

    if (mtx_lock(&mtx) != thrd_success) {
        return 1;
    }
    ready = 1;
    if (cnd_broadcast(&cnd) != thrd_success) {
        return 1;
    }
    mtx_unlock(&mtx);

    if (thrd_join(ta, &ra) != thrd_success) {
        return 1;
    }
    if (thrd_join(tb, &rb) != thrd_success) {
        return 1;
    }
    if (ra != 2 || rb != 3) {
        return 1;
    }

    if (mtx_lock(&mtx) != thrd_success) {
        return 1;
    }
    if (thrd_create(&tt, tryer, 0) != thrd_success) {
        return 1;
    }
    if (thrd_join(tt, 0) != thrd_success) {
        return 1;
    }
    mtx_unlock(&mtx);

    if (mtx_lock(&timed) != thrd_success) {
        return 1;
    }
    if (thrd_create(&tt, timed_tryer, 0) != thrd_success) {
        return 1;
    }
    if (thrd_join(tt, &rt) != thrd_success) {
        return 1;
    }
    mtx_unlock(&timed);

    rec_ok = mtx_lock(&rec) == thrd_success && mtx_lock(&rec) == thrd_success
        && mtx_unlock(&rec) == thrd_success && mtx_unlock(&rec) == thrd_success;
    timed_ok = (try_res == thrd_busy) && (rt == thrd_timedout);

    tiny.tv_sec = 0;
    tiny.tv_nsec = 1;
    sleep_ok = thrd_sleep(&tiny, 0) == 0;
    thrd_yield();
    yield_ok = 1;
    eq = thrd_equal(thrd_current(), thrd_current()) != 0;

    tss_delete(key);
    cnd_destroy(&cnd);
    mtx_destroy(&mtx);
    mtx_destroy(&timed);
    mtx_destroy(&rec);

    printf("%d %d %d %d %d %d %d\n", once_count, ra + rb, rec_ok, timed_ok,
           sleep_ok, eq, yield_ok && TSS_DTOR_ITERATIONS > 0);
    return !(once_count == 1 && rec_ok && timed_ok && sleep_ok && eq
             && yield_ok && TSS_DTOR_ITERATIONS > 0);
}
