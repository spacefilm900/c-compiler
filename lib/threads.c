/* Original C11 <threads.h> runtime for targets whose C library does
   not provide one (macOS, Windows, and a Linux C library without
   <threads.h>). Written from N1570 7.26 and the published POSIX and
   Win32 thread APIs. Not compiled from Clang, GCC, musl, glibc, or
   winpthreads source.

   Do not link this file on a Linux target that already has a C
   library <threads.h>; include/threads.h uses that header, and the
   symbols would clash. The file becomes a no-op in that case. */

/* Quoted path so a Clang oracle can compile this file without
   -I include/, which would otherwise shadow the host <stdatomic.h>
   with this compiler's header. hcc still finds <stdatomic.h> in its
   own include/ directory. */
#include "../include/threads.h"

#ifndef __HCC_THREADS_SHIM

/* The platform C library already provides the thread functions. */
int __hcc_threads_runtime_unused;

#else

#include <stdatomic.h>
#include <stdlib.h>

#define HCC_TSS_MAX 64
#define HCC_ONCE_IDLE 0
#define HCC_ONCE_BUSY 1
#define HCC_ONCE_DONE 2

struct hcc_tss_key {
    int used;
    unsigned gen;
    tss_dtor_t dtor;
};

static struct hcc_tss_key hcc_tss_keys[HCC_TSS_MAX];
static _Atomic int hcc_tss_init_lock;
static thread_local void *hcc_tss_values[HCC_TSS_MAX];
static thread_local unsigned hcc_tss_gen[HCC_TSS_MAX];
static thread_local int hcc_tss_bound;
static _Atomic int hcc_live_threads = 1;

#ifdef _WIN32
#define HCC_THRD_CAP 256
struct hcc_thrd_rec {
    unsigned long long id;
    void *handle;
    int used;
};

static struct hcc_thrd_rec hcc_thrds[HCC_THRD_CAP];
static _Atomic int hcc_thrd_lock;
#endif

struct hcc_start {
    thrd_start_t func;
    void *arg;
    _Atomic int ready;
};

static void hcc_tss_lock(void) {
    int expected;
    for (;;) {
        expected = 0;
        if (atomic_compare_exchange_strong_explicit(
                &hcc_tss_init_lock, &expected, 1,
                memory_order_acquire, memory_order_relaxed)) {
            return;
        }
        thrd_yield();
    }
}

static void hcc_tss_unlock(void) {
    atomic_store_explicit(&hcc_tss_init_lock, 0, memory_order_release);
}

#ifdef _WIN32
static void hcc_thrd_lock_enter(void) {
    int expected;
    for (;;) {
        expected = 0;
        if (atomic_compare_exchange_strong_explicit(
                &hcc_thrd_lock, &expected, 1,
                memory_order_acquire, memory_order_relaxed)) {
            return;
        }
        thrd_yield();
    }
}

static void hcc_thrd_lock_leave(void) {
    atomic_store_explicit(&hcc_thrd_lock, 0, memory_order_release);
}

static int hcc_thrd_reserve(void) {
    int i;
    hcc_thrd_lock_enter();
    for (i = 0; i < HCC_THRD_CAP; i++) {
        if (!hcc_thrds[i].used) {
            hcc_thrds[i].used = 1;
            hcc_thrds[i].id = 0;
            hcc_thrds[i].handle = 0;
            hcc_thrd_lock_leave();
            return i;
        }
    }
    hcc_thrd_lock_leave();
    return -1;
}

static void hcc_thrd_fill(int slot, unsigned long long id, void *handle) {
    hcc_thrd_lock_enter();
    hcc_thrds[slot].id = id;
    hcc_thrds[slot].handle = handle;
    hcc_thrd_lock_leave();
}

static void hcc_thrd_unreserve(int slot) {
    hcc_thrd_lock_enter();
    hcc_thrds[slot].used = 0;
    hcc_thrds[slot].id = 0;
    hcc_thrds[slot].handle = 0;
    hcc_thrd_lock_leave();
}

static void *hcc_thrd_lookup(unsigned long long id) {
    int i;
    void *h = 0;
    hcc_thrd_lock_enter();
    for (i = 0; i < HCC_THRD_CAP; i++) {
        if (hcc_thrds[i].used && hcc_thrds[i].id == id) {
            h = hcc_thrds[i].handle;
            break;
        }
    }
    hcc_thrd_lock_leave();
    return h;
}

static void *hcc_thrd_forget(unsigned long long id) {
    int i;
    void *h = 0;
    hcc_thrd_lock_enter();
    for (i = 0; i < HCC_THRD_CAP; i++) {
        if (hcc_thrds[i].used && hcc_thrds[i].id == id) {
            h = hcc_thrds[i].handle;
            hcc_thrds[i].used = 0;
            hcc_thrds[i].id = 0;
            hcc_thrds[i].handle = 0;
            break;
        }
    }
    hcc_thrd_lock_leave();
    return h;
}

#endif

static void hcc_wait_start(struct hcc_start *s) {
    while (atomic_load_explicit(&s->ready, memory_order_acquire) == 0) {
        thrd_yield();
    }
}

static void hcc_publish_start(struct hcc_start *s) {
    atomic_store_explicit(&s->ready, 1, memory_order_release);
}

static void hcc_thread_ended(void) {
    if (atomic_fetch_sub_explicit(&hcc_live_threads, 1, memory_order_acq_rel) == 1) {
        exit(EXIT_SUCCESS);
    }
}

static void hcc_gate_lock(mtx_t *mtx) {
    int expected;
    for (;;) {
        expected = 0;
        if (atomic_compare_exchange_strong_explicit(
                &mtx->_gate, &expected, 1,
                memory_order_acquire, memory_order_relaxed)) {
            return;
        }
        thrd_yield();
    }
}

static void hcc_gate_unlock(mtx_t *mtx) {
    atomic_store_explicit(&mtx->_gate, 0, memory_order_release);
}

static void hcc_run_tss_dtors(void) {
    int i, k;
    if (!hcc_tss_bound) {
        return;
    }
    for (i = 0; i < TSS_DTOR_ITERATIONS; i++) {
        int any = 0;
        for (k = 0; k < HCC_TSS_MAX; k++) {
            void *val;
            tss_dtor_t dtor;
            unsigned gen;
            hcc_tss_lock();
            if (!hcc_tss_keys[k].used) {
                hcc_tss_unlock();
                continue;
            }
            dtor = hcc_tss_keys[k].dtor;
            gen = hcc_tss_keys[k].gen;
            hcc_tss_unlock();
            if (dtor == 0 || hcc_tss_gen[k] != gen) {
                continue;
            }
            val = hcc_tss_values[k];
            if (val == 0) {
                continue;
            }
            hcc_tss_values[k] = 0;
            dtor(val);
            any = 1;
        }
        if (!any) {
            break;
        }
    }
}

#ifdef _WIN32

/* Win32 typedefs and functions from the published API. On 64-bit
   Windows the calling convention is the single Microsoft x64 ABI. */

typedef void *hcc_handle;
typedef unsigned int hcc_dword;

void InitializeSRWLock(void *);
void AcquireSRWLockExclusive(void *);
unsigned char TryAcquireSRWLockExclusive(void *);
void ReleaseSRWLockExclusive(void *);
void InitializeConditionVariable(void *);
int SleepConditionVariableSRW(void *, void *, unsigned long, unsigned long);
void WakeConditionVariable(void *);
void WakeAllConditionVariable(void *);
hcc_handle CreateThread(void *, unsigned long long,
                        unsigned int (*)(void *), void *,
                        unsigned long, unsigned long *);
unsigned long WaitForSingleObject(hcc_handle, unsigned long);
int CloseHandle(hcc_handle);
int GetExitCodeThread(hcc_handle, unsigned long *);
unsigned long GetCurrentThreadId(void);
void Sleep(unsigned long);
int SwitchToThread(void);
void ExitThread(unsigned long);
void GetSystemTimeAsFileTime(void *);
unsigned long GetLastError(void);
int QueryPerformanceCounter(long long *);
int QueryPerformanceFrequency(long long *);

#define HCC_INFINITE 0xfffffffful
#define HCC_WAIT_TIMEOUT 258ul
#define HCC_WAIT_OBJECT_0 0ul

static int hcc_utc_now(struct timespec *ts) {
    /* FILETIME is 100 ns since 1601-01-01. Unix epoch is 11644473600
       seconds later. */
    unsigned long lo_hi[2];
    unsigned long long t;
    GetSystemTimeAsFileTime((void *)lo_hi);
    t = (unsigned long long)lo_hi[0] | ((unsigned long long)lo_hi[1] << 32);
    if (t < 116444736000000000ull) {
        ts->tv_sec = 0;
        ts->tv_nsec = 0;
        return 1;
    }
    t -= 116444736000000000ull;
    ts->tv_sec = (time_t)(t / 10000000ull);
    ts->tv_nsec = (long)((t % 10000000ull) * 100ull);
    return 1;
}

static unsigned long long hcc_tid(void) {
    return (unsigned long long)GetCurrentThreadId();
}

static void *hcc_mtx_lock_obj(mtx_t *mtx) {
    return (void *)mtx->_storage;
}

static void *hcc_cnd_obj(cnd_t *cond) {
    return (void *)cond->_storage;
}

static int hcc_timespec_le(const struct timespec *a, const struct timespec *b) {
    if (a->tv_sec < b->tv_sec) {
        return 1;
    }
    if (a->tv_sec > b->tv_sec) {
        return 0;
    }
    return a->tv_nsec <= b->tv_nsec;
}

static unsigned long hcc_ms_until(const struct timespec *ts) {
    struct timespec now;
    long long sec, nsec, ms;
    hcc_utc_now(&now);
    if (hcc_timespec_le(ts, &now)) {
        return 0;
    }
    sec = (long long)ts->tv_sec - (long long)now.tv_sec;
    nsec = (long)ts->tv_nsec - (long)now.tv_nsec;
    if (nsec < 0) {
        sec--;
        nsec += 1000000000L;
    }
    if (sec < 0) {
        return 0;
    }
    ms = sec * 1000 + (nsec + 999999L) / 1000000L;
    if (ms <= 0) {
        return 0;
    }
    if (ms >= (long long)HCC_INFINITE) {
        return HCC_INFINITE - 1;
    }
    return (unsigned long)ms;
}

static int hcc_lock_inner(mtx_t *mtx) {
    AcquireSRWLockExclusive(hcc_mtx_lock_obj(mtx));
    return 0;
}

static int hcc_trylock_inner(mtx_t *mtx) {
    return TryAcquireSRWLockExclusive(hcc_mtx_lock_obj(mtx)) ? 0 : 1;
}

static void hcc_unlock_inner(mtx_t *mtx) {
    ReleaseSRWLockExclusive(hcc_mtx_lock_obj(mtx));
}

#define HCC_ERROR_TIMEOUT 1460ul

static unsigned int hcc_win_start(void *p) {
    struct hcc_start *s = (struct hcc_start *)p;
    thrd_start_t func;
    void *arg;
    int res;
    hcc_wait_start(s);
    func = s->func;
    arg = s->arg;
    free(s);
    hcc_tss_bound = 1;
    res = func(arg);
    hcc_run_tss_dtors();
    hcc_thread_ended();
    return (unsigned int)res;
}

int thrd_create(thrd_t *thr, thrd_start_t func, void *arg) {
    struct hcc_start *s;
    hcc_handle h;
    unsigned long id;
    unsigned long err;
    int slot;
    if (thr == 0 || func == 0) {
        return thrd_error;
    }
    s = (struct hcc_start *)malloc(sizeof(*s));
    if (s == 0) {
        return thrd_nomem;
    }
    s->func = func;
    s->arg = arg;
    atomic_store_explicit(&s->ready, 0, memory_order_relaxed);
    slot = hcc_thrd_reserve();
    if (slot < 0) {
        free(s);
        return thrd_nomem;
    }
    h = CreateThread(0, 0, hcc_win_start, s, 0, &id);
    if (h == 0) {
        err = GetLastError();
        hcc_thrd_unreserve(slot);
        free(s);
        return err == 8ul ? thrd_nomem : thrd_error;
    }
    hcc_thrd_fill(slot, (unsigned long long)id, (void *)h);
    atomic_fetch_add_explicit(&hcc_live_threads, 1, memory_order_relaxed);
    thr->_id = (unsigned long long)id;
    thr->_handle = (void *)h;
    hcc_publish_start(s);
    return thrd_success;
}

thrd_t thrd_current(void) {
    thrd_t t;
    t._id = hcc_tid();
    t._handle = hcc_thrd_lookup(t._id);
    return t;
}

int thrd_detach(thrd_t thr) {
    void *h = thr._handle ? thr._handle : hcc_thrd_lookup(thr._id);
    if (h == 0) {
        return thrd_error;
    }
    hcc_thrd_forget(thr._id);
    return CloseHandle((hcc_handle)h) ? thrd_success : thrd_error;
}

int thrd_equal(thrd_t thr0, thrd_t thr1) {
    return thr0._id == thr1._id;
}

_Noreturn void thrd_exit(int res) {
    hcc_run_tss_dtors();
    if (atomic_fetch_sub_explicit(&hcc_live_threads, 1, memory_order_acq_rel) == 1) {
        exit(EXIT_SUCCESS);
    }
    ExitThread((unsigned long)(unsigned int)res);
    for (;;) {
    }
}

int thrd_join(thrd_t thr, int *res) {
    unsigned long code;
    void *h = thr._handle ? thr._handle : hcc_thrd_lookup(thr._id);
    if (h == 0) {
        return thrd_error;
    }
    if (WaitForSingleObject((hcc_handle)h, HCC_INFINITE) != HCC_WAIT_OBJECT_0) {
        return thrd_error;
    }
    if (res != 0) {
        if (!GetExitCodeThread((hcc_handle)h, &code)) {
            hcc_thrd_forget(thr._id);
            CloseHandle((hcc_handle)h);
            return thrd_error;
        }
        *res = (int)code;
    }
    hcc_thrd_forget(thr._id);
    return CloseHandle((hcc_handle)h) ? thrd_success : thrd_error;
}

int thrd_sleep(const struct timespec *duration, struct timespec *remaining) {
    long long freq, start, now;
    unsigned long long need, have;
    (void)remaining;
    if (duration == 0 || duration->tv_sec < 0 || duration->tv_nsec < 0
        || duration->tv_nsec >= 1000000000L) {
        return -2;
    }
    if (!QueryPerformanceFrequency(&freq) || freq <= 0 || !QueryPerformanceCounter(&start)) {
        return -2;
    }
    need = (unsigned long long)duration->tv_sec * (unsigned long long)freq
        + ((unsigned long long)duration->tv_nsec * (unsigned long long)freq) / 1000000000ull;
    for (;;) {
        unsigned long long left;
        unsigned long ms;
        if (!QueryPerformanceCounter(&now)) {
            return -2;
        }
        have = (unsigned long long)(now - start);
        if (have >= need) {
            return 0;
        }
        left = need - have;
        ms = (unsigned long)((left * 1000ull) / (unsigned long long)freq);
        if (ms == 0) {
            ms = 1;
        }
        if (ms >= HCC_INFINITE) {
            ms = HCC_INFINITE - 1;
        }
        Sleep(ms);
    }
}

void thrd_yield(void) {
    SwitchToThread();
}

int mtx_init(mtx_t *mtx, int type) {
    int rec, timed;
    if (mtx == 0) {
        return thrd_error;
    }
    rec = type & mtx_recursive;
    timed = type & mtx_timed;
    if ((type & ~(mtx_plain | mtx_recursive | mtx_timed)) != 0) {
        return thrd_error;
    }
    if (!rec && !timed && type != mtx_plain) {
        return thrd_error;
    }
    if ((type & mtx_plain) == 0 && !rec && !timed) {
        return thrd_error;
    }
    /* Accept the four combinations in 7.26.4.2, and mtx_recursive
       alone as a recursive mutex without timeout. */
    if (type != mtx_plain && type != mtx_timed && type != mtx_recursive
        && type != (mtx_plain | mtx_recursive)
        && type != (mtx_timed | mtx_recursive)
        && type != (mtx_plain | mtx_timed)
        && type != (mtx_plain | mtx_timed | mtx_recursive)) {
        return thrd_error;
    }
    InitializeSRWLock(hcc_mtx_lock_obj(mtx));
    mtx->_type = type;
    mtx->_count = 0;
    mtx->_owner = 0;
    mtx->_gate = 0;
    return thrd_success;
}

void mtx_destroy(mtx_t *mtx) {
    (void)mtx;
}

static int hcc_mtx_try_own(mtx_t *mtx, unsigned long long self) {
    hcc_gate_lock(mtx);
    if ((mtx->_type & mtx_recursive) && mtx->_count > 0 && mtx->_owner == self) {
        mtx->_count++;
        hcc_gate_unlock(mtx);
        return thrd_success;
    }
    hcc_gate_unlock(mtx);
    if (hcc_trylock_inner(mtx) != 0) {
        return thrd_busy;
    }
    hcc_gate_lock(mtx);
    mtx->_owner = self;
    mtx->_count = 1;
    hcc_gate_unlock(mtx);
    return thrd_success;
}

static int hcc_mtx_acquire(mtx_t *mtx, int try_only) {
    unsigned long long self = hcc_tid();
    int r;
    if (mtx == 0) {
        return thrd_error;
    }
    for (;;) {
        r = hcc_mtx_try_own(mtx, self);
        if (r == thrd_success) {
            return thrd_success;
        }
        if (try_only) {
            return thrd_busy;
        }
        if (hcc_lock_inner(mtx) != 0) {
            return thrd_error;
        }
        hcc_gate_lock(mtx);
        mtx->_owner = self;
        mtx->_count = 1;
        hcc_gate_unlock(mtx);
        return thrd_success;
    }
}

int mtx_lock(mtx_t *mtx) {
    return hcc_mtx_acquire(mtx, 0);
}

int mtx_trylock(mtx_t *mtx) {
    return hcc_mtx_acquire(mtx, 1);
}

int mtx_timedlock(mtx_t *restrict mtx, const struct timespec *restrict ts) {
    unsigned long long self;
    int first;
    if (mtx == 0 || ts == 0) {
        return thrd_error;
    }
    if ((mtx->_type & mtx_timed) == 0) {
        return thrd_error;
    }
    self = hcc_tid();
    first = 1;
    for (;;) {
        struct timespec now;
        int r;
        if (!first) {
            hcc_utc_now(&now);
            if (hcc_timespec_le(ts, &now)) {
                return thrd_timedout;
            }
        }
        r = hcc_mtx_try_own(mtx, self);
        if (r == thrd_success) {
            return thrd_success;
        }
        hcc_utc_now(&now);
        if (hcc_timespec_le(ts, &now)) {
            return thrd_timedout;
        }
        first = 0;
        Sleep(1);
    }
}

int mtx_unlock(mtx_t *mtx) {
    int last;
    if (mtx == 0) {
        return thrd_error;
    }
    hcc_gate_lock(mtx);
    if (mtx->_count <= 0 || mtx->_owner != hcc_tid()) {
        hcc_gate_unlock(mtx);
        return thrd_error;
    }
    mtx->_count--;
    last = mtx->_count == 0;
    if (last) {
        mtx->_owner = 0;
    }
    hcc_gate_unlock(mtx);
    if (last) {
        hcc_unlock_inner(mtx);
    }
    return thrd_success;
}

int cnd_init(cnd_t *cond) {
    if (cond == 0) {
        return thrd_error;
    }
    InitializeConditionVariable(hcc_cnd_obj(cond));
    return thrd_success;
}

void cnd_destroy(cnd_t *cond) {
    (void)cond;
}

int cnd_signal(cnd_t *cond) {
    if (cond == 0) {
        return thrd_error;
    }
    WakeConditionVariable(hcc_cnd_obj(cond));
    return thrd_success;
}

int cnd_broadcast(cnd_t *cond) {
    if (cond == 0) {
        return thrd_error;
    }
    WakeAllConditionVariable(hcc_cnd_obj(cond));
    return thrd_success;
}

static int hcc_cnd_wait(cnd_t *cond, mtx_t *mtx, const struct timespec *ts) {
    int count;
    unsigned long long owner;
    unsigned long ms;
    int ok;
    if (cond == 0 || mtx == 0) {
        return thrd_error;
    }
    hcc_gate_lock(mtx);
    if (mtx->_count <= 0 || mtx->_owner != hcc_tid()) {
        hcc_gate_unlock(mtx);
        return thrd_error;
    }
    count = mtx->_count;
    owner = mtx->_owner;
    mtx->_count = 0;
    mtx->_owner = 0;
    hcc_gate_unlock(mtx);
    for (;;) {
        unsigned long err;
        ms = ts ? hcc_ms_until(ts) : HCC_INFINITE;
        if (ts && ms == 0) {
            hcc_gate_lock(mtx);
            mtx->_owner = owner;
            mtx->_count = count;
            hcc_gate_unlock(mtx);
            return thrd_timedout;
        }
        if (ms >= HCC_INFINITE) {
            ms = ts ? (HCC_INFINITE - 1) : HCC_INFINITE;
        }
        ok = SleepConditionVariableSRW(hcc_cnd_obj(cond), hcc_mtx_lock_obj(mtx),
                                       ms, 0);
        if (ok) {
            hcc_gate_lock(mtx);
            mtx->_owner = owner;
            mtx->_count = count;
            hcc_gate_unlock(mtx);
            return thrd_success;
        }
        err = GetLastError();
        if (err == HCC_ERROR_TIMEOUT) {
            if (ts && hcc_ms_until(ts) == 0) {
                hcc_gate_lock(mtx);
                mtx->_owner = owner;
                mtx->_count = count;
                hcc_gate_unlock(mtx);
                return thrd_timedout;
            }
            if (ts) {
                continue;
            }
        }
        hcc_gate_lock(mtx);
        mtx->_owner = owner;
        mtx->_count = count;
        hcc_gate_unlock(mtx);
        return thrd_error;
    }
}

int cnd_wait(cnd_t *cond, mtx_t *mtx) {
    return hcc_cnd_wait(cond, mtx, 0);
}

int cnd_timedwait(cnd_t *restrict cond, mtx_t *restrict mtx,
                  const struct timespec *restrict ts) {
    if (ts == 0) {
        return thrd_error;
    }
    return hcc_cnd_wait(cond, mtx, ts);
}

#else /* POSIX pthreads */

#include <errno.h>

typedef unsigned long long hcc_thread_t;
typedef struct {
    _Alignas(16) unsigned char _s[256];
} hcc_mutex;
typedef struct {
    _Alignas(16) unsigned char _s[256];
} hcc_cond;

int pthread_create(hcc_thread_t *, const void *, void *(*)(void *), void *);
int pthread_join(hcc_thread_t, void **);
int pthread_detach(hcc_thread_t);
hcc_thread_t pthread_self(void);
int pthread_equal(hcc_thread_t, hcc_thread_t);
void pthread_exit(void *);
int pthread_mutex_init(hcc_mutex *, const void *);
int pthread_mutex_destroy(hcc_mutex *);
int pthread_mutex_lock(hcc_mutex *);
int pthread_mutex_trylock(hcc_mutex *);
int pthread_mutex_unlock(hcc_mutex *);
int pthread_cond_init(hcc_cond *, const void *);
int pthread_cond_destroy(hcc_cond *);
int pthread_cond_signal(hcc_cond *);
int pthread_cond_broadcast(hcc_cond *);
int pthread_cond_wait(hcc_cond *, hcc_mutex *);
int pthread_cond_timedwait(hcc_cond *, hcc_mutex *, const struct timespec *);
int nanosleep(const struct timespec *, struct timespec *);
int sched_yield(void);
int timespec_get(struct timespec *ts, int base);

static hcc_mutex *hcc_mtx_lock_obj(mtx_t *mtx) {
    return (hcc_mutex *)mtx->_storage;
}

static hcc_cond *hcc_cnd_obj(cnd_t *cond) {
    return (hcc_cond *)cond->_storage;
}

static unsigned long long hcc_tid(void) {
    return (unsigned long long)pthread_self();
}

static int hcc_utc_now(struct timespec *ts) {
    return timespec_get(ts, TIME_UTC) != 0;
}

static int hcc_timespec_le(const struct timespec *a, const struct timespec *b) {
    if (a->tv_sec < b->tv_sec) {
        return 1;
    }
    if (a->tv_sec > b->tv_sec) {
        return 0;
    }
    return a->tv_nsec <= b->tv_nsec;
}

static int hcc_lock_inner(mtx_t *mtx) {
    return pthread_mutex_lock(hcc_mtx_lock_obj(mtx));
}

static int hcc_trylock_inner(mtx_t *mtx) {
    return pthread_mutex_trylock(hcc_mtx_lock_obj(mtx));
}

static void hcc_unlock_inner(mtx_t *mtx) {
    pthread_mutex_unlock(hcc_mtx_lock_obj(mtx));
}

static void *hcc_posix_start(void *p) {
    struct hcc_start *s = (struct hcc_start *)p;
    thrd_start_t func;
    void *arg;
    int res;
    hcc_wait_start(s);
    func = s->func;
    arg = s->arg;
    free(s);
    hcc_tss_bound = 1;
    res = func(arg);
    hcc_run_tss_dtors();
    hcc_thread_ended();
    return (void *)(long)res;
}

int thrd_create(thrd_t *thr, thrd_start_t func, void *arg) {
    struct hcc_start *s;
    hcc_thread_t id;
    int err;
    if (thr == 0 || func == 0) {
        return thrd_error;
    }
    s = (struct hcc_start *)malloc(sizeof(*s));
    if (s == 0) {
        return thrd_nomem;
    }
    s->func = func;
    s->arg = arg;
    atomic_store_explicit(&s->ready, 0, memory_order_relaxed);
    err = pthread_create(&id, 0, hcc_posix_start, s);
    if (err != 0) {
        free(s);
        return thrd_error;
    }
    atomic_fetch_add_explicit(&hcc_live_threads, 1, memory_order_relaxed);
    thr->_id = (unsigned long long)id;
    thr->_handle = 0;
    hcc_publish_start(s);
    return thrd_success;
}

thrd_t thrd_current(void) {
    thrd_t t;
    t._id = hcc_tid();
    t._handle = 0;
    return t;
}

int thrd_detach(thrd_t thr) {
    return pthread_detach((hcc_thread_t)thr._id) == 0 ? thrd_success : thrd_error;
}

int thrd_equal(thrd_t thr0, thrd_t thr1) {
    return pthread_equal((hcc_thread_t)thr0._id, (hcc_thread_t)thr1._id) != 0;
}

_Noreturn void thrd_exit(int res) {
    hcc_run_tss_dtors();
    if (atomic_fetch_sub_explicit(&hcc_live_threads, 1, memory_order_acq_rel) == 1) {
        exit(EXIT_SUCCESS);
    }
    pthread_exit((void *)(long)res);
    for (;;) {
    }
}

int thrd_join(thrd_t thr, int *res) {
    void *code = 0;
    if (pthread_join((hcc_thread_t)thr._id, &code) != 0) {
        return thrd_error;
    }
    if (res != 0) {
        *res = (int)(long)code;
    }
    return thrd_success;
}

int thrd_sleep(const struct timespec *duration, struct timespec *remaining) {
    if (duration == 0 || duration->tv_sec < 0 || duration->tv_nsec < 0
        || duration->tv_nsec >= 1000000000L) {
        return -2;
    }
    if (nanosleep(duration, remaining) == 0) {
        return 0;
    }
    return -1;
}

void thrd_yield(void) {
    sched_yield();
}

int mtx_init(mtx_t *mtx, int type) {
    if (mtx == 0) {
        return thrd_error;
    }
    if (type != mtx_plain && type != mtx_timed && type != mtx_recursive
        && type != (mtx_plain | mtx_recursive)
        && type != (mtx_timed | mtx_recursive)
        && type != (mtx_plain | mtx_timed)
        && type != (mtx_plain | mtx_timed | mtx_recursive)) {
        return thrd_error;
    }
    if (pthread_mutex_init(hcc_mtx_lock_obj(mtx), 0) != 0) {
        return thrd_error;
    }
    mtx->_type = type;
    mtx->_count = 0;
    mtx->_owner = 0;
    mtx->_gate = 0;
    return thrd_success;
}

void mtx_destroy(mtx_t *mtx) {
    if (mtx != 0) {
        pthread_mutex_destroy(hcc_mtx_lock_obj(mtx));
    }
}

static int hcc_mtx_try_own(mtx_t *mtx, unsigned long long self) {
    hcc_gate_lock(mtx);
    if ((mtx->_type & mtx_recursive) && mtx->_count > 0 && mtx->_owner == self) {
        mtx->_count++;
        hcc_gate_unlock(mtx);
        return thrd_success;
    }
    hcc_gate_unlock(mtx);
    if (hcc_trylock_inner(mtx) != 0) {
        return thrd_busy;
    }
    hcc_gate_lock(mtx);
    mtx->_owner = self;
    mtx->_count = 1;
    hcc_gate_unlock(mtx);
    return thrd_success;
}

static int hcc_mtx_acquire(mtx_t *mtx, int try_only) {
    unsigned long long self = hcc_tid();
    int r;
    if (mtx == 0) {
        return thrd_error;
    }
    for (;;) {
        r = hcc_mtx_try_own(mtx, self);
        if (r == thrd_success) {
            return thrd_success;
        }
        if (try_only) {
            return thrd_busy;
        }
        if (hcc_lock_inner(mtx) != 0) {
            return thrd_error;
        }
        hcc_gate_lock(mtx);
        mtx->_owner = self;
        mtx->_count = 1;
        hcc_gate_unlock(mtx);
        return thrd_success;
    }
}

int mtx_lock(mtx_t *mtx) {
    return hcc_mtx_acquire(mtx, 0);
}

int mtx_trylock(mtx_t *mtx) {
    return hcc_mtx_acquire(mtx, 1);
}

int mtx_timedlock(mtx_t *restrict mtx, const struct timespec *restrict ts) {
    unsigned long long self;
    int first;
    if (mtx == 0 || ts == 0) {
        return thrd_error;
    }
    if ((mtx->_type & mtx_timed) == 0) {
        return thrd_error;
    }
    self = hcc_tid();
    first = 1;
    for (;;) {
        struct timespec now;
        int r;
        if (!first) {
            if (!hcc_utc_now(&now) || hcc_timespec_le(ts, &now)) {
                return thrd_timedout;
            }
        }
        r = hcc_mtx_try_own(mtx, self);
        if (r == thrd_success) {
            return thrd_success;
        }
        if (!hcc_utc_now(&now) || hcc_timespec_le(ts, &now)) {
            return thrd_timedout;
        }
        first = 0;
        {
            struct timespec sl;
            sl.tv_sec = 0;
            sl.tv_nsec = 1000000L;
            nanosleep(&sl, 0);
        }
    }
}

int mtx_unlock(mtx_t *mtx) {
    int last;
    if (mtx == 0) {
        return thrd_error;
    }
    hcc_gate_lock(mtx);
    if (mtx->_count <= 0 || mtx->_owner != hcc_tid()) {
        hcc_gate_unlock(mtx);
        return thrd_error;
    }
    mtx->_count--;
    last = mtx->_count == 0;
    if (last) {
        mtx->_owner = 0;
    }
    hcc_gate_unlock(mtx);
    if (last) {
        hcc_unlock_inner(mtx);
    }
    return thrd_success;
}

int cnd_init(cnd_t *cond) {
    if (cond == 0) {
        return thrd_error;
    }
    if (pthread_cond_init(hcc_cnd_obj(cond), 0) != 0) {
        return thrd_nomem;
    }
    return thrd_success;
}

void cnd_destroy(cnd_t *cond) {
    if (cond != 0) {
        pthread_cond_destroy(hcc_cnd_obj(cond));
    }
}

int cnd_signal(cnd_t *cond) {
    if (cond == 0) {
        return thrd_error;
    }
    return pthread_cond_signal(hcc_cnd_obj(cond)) == 0 ? thrd_success : thrd_error;
}

int cnd_broadcast(cnd_t *cond) {
    if (cond == 0) {
        return thrd_error;
    }
    return pthread_cond_broadcast(hcc_cnd_obj(cond)) == 0 ? thrd_success
                                                          : thrd_error;
}

static int hcc_cnd_wait(cnd_t *cond, mtx_t *mtx, const struct timespec *ts) {
    int count, err;
    unsigned long long owner;
    if (cond == 0 || mtx == 0) {
        return thrd_error;
    }
    hcc_gate_lock(mtx);
    if (mtx->_count <= 0 || mtx->_owner != hcc_tid()) {
        hcc_gate_unlock(mtx);
        return thrd_error;
    }
    count = mtx->_count;
    owner = mtx->_owner;
    mtx->_count = 0;
    mtx->_owner = 0;
    hcc_gate_unlock(mtx);
    if (ts == 0) {
        err = pthread_cond_wait(hcc_cnd_obj(cond), hcc_mtx_lock_obj(mtx));
    } else {
        err = pthread_cond_timedwait(hcc_cnd_obj(cond), hcc_mtx_lock_obj(mtx), ts);
    }
    hcc_gate_lock(mtx);
    mtx->_owner = owner;
    mtx->_count = count;
    hcc_gate_unlock(mtx);
    if (err == 0) {
        return thrd_success;
    }
    if (ts != 0 && err == ETIMEDOUT) {
        return thrd_timedout;
    }
    return thrd_error;
}

int cnd_wait(cnd_t *cond, mtx_t *mtx) {
    return hcc_cnd_wait(cond, mtx, 0);
}

int cnd_timedwait(cnd_t *restrict cond, mtx_t *restrict mtx,
                  const struct timespec *restrict ts) {
    if (ts == 0) {
        return thrd_error;
    }
    return hcc_cnd_wait(cond, mtx, ts);
}

#endif /* _WIN32 */

void call_once(once_flag *flag, void (*func)(void)) {
    int state, expected;
    if (flag == 0 || func == 0) {
        return;
    }
    state = atomic_load_explicit(&flag->_state, memory_order_acquire);
    if (state == HCC_ONCE_DONE) {
        return;
    }
    expected = HCC_ONCE_IDLE;
    if (atomic_compare_exchange_strong_explicit(
            &flag->_state, &expected, HCC_ONCE_BUSY,
            memory_order_acq_rel, memory_order_acquire)) {
        func();
        atomic_store_explicit(&flag->_state, HCC_ONCE_DONE, memory_order_release);
        return;
    }
    for (;;) {
        state = atomic_load_explicit(&flag->_state, memory_order_acquire);
        if (state == HCC_ONCE_DONE) {
            return;
        }
        thrd_yield();
    }
}

int tss_create(tss_t *key, tss_dtor_t dtor) {
    int i;
    if (key == 0) {
        return thrd_error;
    }
    hcc_tss_lock();
    for (i = 0; i < HCC_TSS_MAX; i++) {
        if (!hcc_tss_keys[i].used) {
            if (hcc_tss_keys[i].gen == 0xffffffffu) {
                continue;
            }
            hcc_tss_keys[i].used = 1;
            hcc_tss_keys[i].gen++;
            if (hcc_tss_keys[i].gen == 0) {
                hcc_tss_keys[i].gen = 1;
            }
            hcc_tss_keys[i].dtor = dtor;
            hcc_tss_unlock();
            *key = (tss_t)i;
            return thrd_success;
        }
    }
    hcc_tss_unlock();
    return thrd_error;
}

void tss_delete(tss_t key) {
    if (key >= (tss_t)HCC_TSS_MAX) {
        return;
    }
    hcc_tss_lock();
    hcc_tss_keys[key].used = 0;
    hcc_tss_keys[key].dtor = 0;
    if (hcc_tss_keys[key].gen != 0xffffffffu) {
        hcc_tss_keys[key].gen++;
    }
    hcc_tss_unlock();
}

void *tss_get(tss_t key) {
    unsigned gen;
    if (key >= (tss_t)HCC_TSS_MAX) {
        return 0;
    }
    hcc_tss_lock();
    if (!hcc_tss_keys[key].used) {
        hcc_tss_unlock();
        return 0;
    }
    gen = hcc_tss_keys[key].gen;
    hcc_tss_unlock();
    if (hcc_tss_gen[key] != gen) {
        return 0;
    }
    return hcc_tss_values[key];
}

int tss_set(tss_t key, void *val) {
    unsigned gen;
    if (key >= (tss_t)HCC_TSS_MAX) {
        return thrd_error;
    }
    hcc_tss_lock();
    if (!hcc_tss_keys[key].used) {
        hcc_tss_unlock();
        return thrd_error;
    }
    gen = hcc_tss_keys[key].gen;
    hcc_tss_unlock();
    hcc_tss_values[key] = val;
    hcc_tss_gen[key] = gen;
    hcc_tss_bound = 1;
    return thrd_success;
}

#endif /* __HCC_THREADS_SHIM */
