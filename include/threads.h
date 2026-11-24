/* Compiler-provided <threads.h> (C11 7.26).

   On a Linux target, if the C library already provides <threads.h>
   (glibc 2.28 and later), this header includes it with #include_next
   so the typedefs and enumeration values come from the platform. Link
   the program with -pthread.

   On macOS and Windows, and on a Linux target whose C library has no
   <threads.h>, this header supplies the typedefs, macros, and
   prototypes, and lib/threads.c is a small original runtime over
   pthreads (POSIX) or the Win32 thread API (Windows). Compile and
   link lib/threads.c with the program, and on POSIX also pass
   -pthread (hcc --link does both). The shim does not include <pthread.h>, because this
   compiler rejects glibc's aligned typedef of pthread typedefs; the
   needed POSIX functions are declared in the runtime with a
   layout-tolerant storage typ for mutexes and condition variables.

   __STDC_NO_THREADS__ is not defined: every target this compiler
   claims has a complete <threads.h>. */

#if !defined(__HCC_THREADS_FORCE_SHIM) && defined(__linux__) && defined(__has_include_next) && __has_include_next(<threads.h>)
#include <time.h>
#include_next <threads.h>
#else

#ifndef _THREADS_H
#define _THREADS_H

#include <time.h>

#define __HCC_THREADS_SHIM 1

#ifndef thread_local
#define thread_local _Thread_local
#endif

#define ONCE_FLAG_INIT {0}
#define TSS_DTOR_ITERATIONS 4

enum {
    mtx_plain = 1,
    mtx_recursive = 2,
    mtx_timed = 4,
    thrd_success = 0,
    thrd_timedout = 1,
    thrd_busy = 2,
    thrd_error = 3,
    thrd_nomem = 4
};

typedef struct {
    _Alignas(16) unsigned char _storage[256];
    int _type;
    int _count;
    unsigned long long _owner;
    _Atomic int _gate;
} mtx_t;

typedef struct {
    _Alignas(16) unsigned char _storage[256];
} cnd_t;

typedef struct {
    unsigned long long _id;
    void *_handle;
} thrd_t;

typedef unsigned tss_t;
typedef void (*tss_dtor_t)(void *);
typedef int (*thrd_start_t)(void *);

typedef struct {
    _Atomic int _state;
} once_flag;

void call_once(once_flag *flag, void (*func)(void));

int cnd_broadcast(cnd_t *cond);
void cnd_destroy(cnd_t *cond);
int cnd_init(cnd_t *cond);
int cnd_signal(cnd_t *cond);
int cnd_timedwait(cnd_t *restrict cond, mtx_t *restrict mtx,
                  const struct timespec *restrict ts);
int cnd_wait(cnd_t *cond, mtx_t *mtx);

void mtx_destroy(mtx_t *mtx);
int mtx_init(mtx_t *mtx, int type);
int mtx_lock(mtx_t *mtx);
int mtx_timedlock(mtx_t *restrict mtx, const struct timespec *restrict ts);
int mtx_trylock(mtx_t *mtx);
int mtx_unlock(mtx_t *mtx);

int thrd_create(thrd_t *thr, thrd_start_t func, void *arg);
thrd_t thrd_current(void);
int thrd_detach(thrd_t thr);
int thrd_equal(thrd_t thr0, thrd_t thr1);
_Noreturn void thrd_exit(int res);
int thrd_join(thrd_t thr, int *res);
int thrd_sleep(const struct timespec *duration, struct timespec *remaining);
void thrd_yield(void);

int tss_create(tss_t *key, tss_dtor_t dtor);
void tss_delete(tss_t key);
void *tss_get(tss_t key);
int tss_set(tss_t key, void *val);

#endif /* _THREADS_H */
#endif /* system <threads.h> */
