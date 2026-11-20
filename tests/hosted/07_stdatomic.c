/* <stdatomic.h> (C11 7.17): memory_order, typedefs, generic functions,
   atomic_flag, and fences. Compared with Clang as a black-box oracle. */

#include <stdatomic.h>
#include <stdint.h>

int printf(const char *, ...);

_Static_assert(ATOMIC_INT_LOCK_FREE == 2, "int is lock-free");
_Static_assert(ATOMIC_POINTER_LOCK_FREE == 2, "pointers are lock-free");
_Static_assert(memory_order_relaxed == 0, "relaxed");
_Static_assert(memory_order_seq_cst == 5, "seq_cst");
_Static_assert(sizeof(atomic_flag) == 1, "atomic_flag size");

int main(void)
{
    atomic_int x = ATOMIC_VAR_INIT(10);
    atomic_init(&x, 4);
    atomic_store_explicit(&x, 7, memory_order_relaxed);
    int v = atomic_load_explicit(&x, memory_order_acquire);
    int old = atomic_exchange_explicit(&x, 1, memory_order_acq_rel);
    int e = 1;
    int ok = atomic_compare_exchange_strong_explicit(
        &x, &e, 20, memory_order_seq_cst, memory_order_relaxed);
    int added = atomic_fetch_add_explicit(&x, 3, memory_order_release);
    int ored = atomic_fetch_or(&x, 8);
    int anded = atomic_fetch_and(&x, 31);
    int xored = atomic_fetch_xor(&x, 1);
    int subed = atomic_fetch_sub(&x, 2);
    atomic_thread_fence(memory_order_seq_cst);
    atomic_signal_fence(memory_order_acquire);
    atomic_flag f = ATOMIC_FLAG_INIT;
    int set = atomic_flag_test_and_set(&f);
    atomic_flag_clear_explicit(&f, memory_order_release);
    int set2 = atomic_flag_test_and_set_explicit(&f, memory_order_relaxed);
    int lf = atomic_is_lock_free(&x);
    int y = kill_dependency(v);
    atomic_intptr_t ip = 0;
    atomic_store(&ip, (intptr_t)&x);
    printf("%d %d %d %d %d %d %d %d %d %d %d %d %d %d\n",
           v, old, ok, e, added, ored, anded, xored, subed,
           atomic_load(&x), set, set2, lf, y);
    return (v == 7 && old == 7 && ok == 1 && added == 20 && ored == 23
            && anded == 31 && xored == 31 && subed == 30
            && atomic_load(&x) == 28 && set == 0 && set2 == 0 && lf == 1
            && y == 7 && atomic_load(&ip) == (intptr_t)&x)
               ? 0
               : 1;
}
