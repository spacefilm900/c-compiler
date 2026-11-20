/* Compiler-provided <stdatomic.h> (C11 7.17).

   Lock-free atomic object sizes are 1, 2, 4, 8, and 16 bytes on every
   target this compiler supports. Larger atomic types are a diagnostic;
   there is no locking runtime and libatomic is not used.

   memory_order_consume is lowered as acquire (LLVM has no consume).
   A memory_order argument that is not a constant is lowered as
   memory_order_seq_cst.

   The numeric memory_order values 0 through 5 follow the published
   GCC/libatomic ABI. */
#ifndef _STDATOMIC_H
#define _STDATOMIC_H

#include <stddef.h>
#if defined(__has_include) && __has_include(<stdint.h>)
#include <stdint.h>
#define __HCC_STDATOMIC_HAS_STDINT 1
#endif

#define ATOMIC_BOOL_LOCK_FREE 2
#define ATOMIC_CHAR_LOCK_FREE 2
#define ATOMIC_CHAR16_T_LOCK_FREE 2
#define ATOMIC_CHAR32_T_LOCK_FREE 2
#define ATOMIC_WCHAR_T_LOCK_FREE 2
#define ATOMIC_SHORT_LOCK_FREE 2
#define ATOMIC_INT_LOCK_FREE 2
#define ATOMIC_LONG_LOCK_FREE 2
#define ATOMIC_LLONG_LOCK_FREE 2
#define ATOMIC_POINTER_LOCK_FREE 2

typedef enum {
    memory_order_relaxed = 0,
    memory_order_consume = 1,
    memory_order_acquire = 2,
    memory_order_release = 3,
    memory_order_acq_rel = 4,
    memory_order_seq_cst = 5
} memory_order;

#define ATOMIC_VAR_INIT(value) (value)
#define kill_dependency(y) (y)

typedef _Atomic _Bool atomic_bool;
typedef _Atomic char atomic_char;
typedef _Atomic signed char atomic_schar;
typedef _Atomic unsigned char atomic_uchar;
typedef _Atomic short atomic_short;
typedef _Atomic unsigned short atomic_ushort;
typedef _Atomic int atomic_int;
typedef _Atomic unsigned int atomic_uint;
typedef _Atomic long atomic_long;
typedef _Atomic unsigned long atomic_ulong;
typedef _Atomic long long atomic_llong;
typedef _Atomic unsigned long long atomic_ullong;
typedef _Atomic wchar_t atomic_wchar_t;
typedef _Atomic size_t atomic_size_t;
typedef _Atomic ptrdiff_t atomic_ptrdiff_t;

#ifdef __HCC_STDATOMIC_HAS_STDINT
typedef _Atomic uint_least16_t atomic_char16_t;
typedef _Atomic uint_least32_t atomic_char32_t;
typedef _Atomic int_least8_t atomic_int_least8_t;
typedef _Atomic uint_least8_t atomic_uint_least8_t;
typedef _Atomic int_least16_t atomic_int_least16_t;
typedef _Atomic uint_least16_t atomic_uint_least16_t;
typedef _Atomic int_least32_t atomic_int_least32_t;
typedef _Atomic uint_least32_t atomic_uint_least32_t;
typedef _Atomic int_least64_t atomic_int_least64_t;
typedef _Atomic uint_least64_t atomic_uint_least64_t;
typedef _Atomic int_fast8_t atomic_int_fast8_t;
typedef _Atomic uint_fast8_t atomic_uint_fast8_t;
typedef _Atomic int_fast16_t atomic_int_fast16_t;
typedef _Atomic uint_fast16_t atomic_uint_fast16_t;
typedef _Atomic int_fast32_t atomic_int_fast32_t;
typedef _Atomic uint_fast32_t atomic_uint_fast32_t;
typedef _Atomic int_fast64_t atomic_int_fast64_t;
typedef _Atomic uint_fast64_t atomic_uint_fast64_t;
typedef _Atomic intptr_t atomic_intptr_t;
typedef _Atomic uintptr_t atomic_uintptr_t;
typedef _Atomic intmax_t atomic_intmax_t;
typedef _Atomic uintmax_t atomic_uintmax_t;
#else
/* No <stdint.h> on this include path (a unit-test or freestanding
   environment). The typedefs use fundamental types of the same width;
   intptr and uintptr follow ptrdiff_t and size_t. */
typedef _Atomic unsigned short atomic_char16_t;
typedef _Atomic unsigned int atomic_char32_t;
typedef _Atomic signed char atomic_int_least8_t;
typedef _Atomic unsigned char atomic_uint_least8_t;
typedef _Atomic short atomic_int_least16_t;
typedef _Atomic unsigned short atomic_uint_least16_t;
typedef _Atomic int atomic_int_least32_t;
typedef _Atomic unsigned int atomic_uint_least32_t;
typedef _Atomic long long atomic_int_least64_t;
typedef _Atomic unsigned long long atomic_uint_least64_t;
typedef _Atomic signed char atomic_int_fast8_t;
typedef _Atomic unsigned char atomic_uint_fast8_t;
typedef _Atomic int atomic_int_fast16_t;
typedef _Atomic unsigned int atomic_uint_fast16_t;
typedef _Atomic int atomic_int_fast32_t;
typedef _Atomic unsigned int atomic_uint_fast32_t;
typedef _Atomic long long atomic_int_fast64_t;
typedef _Atomic unsigned long long atomic_uint_fast64_t;
typedef _Atomic ptrdiff_t atomic_intptr_t;
typedef _Atomic size_t atomic_uintptr_t;
typedef _Atomic long long atomic_intmax_t;
typedef _Atomic unsigned long long atomic_uintmax_t;
#endif

typedef struct {
    _Atomic unsigned char __val;
} atomic_flag;

#define ATOMIC_FLAG_INIT {0}

#define atomic_init(obj, value) __hcc_atomic_init((obj), (value))

#define atomic_thread_fence(order) __hcc_atomic_thread_fence(order)
#define atomic_signal_fence(order) __hcc_atomic_signal_fence(order)
#define atomic_is_lock_free(obj) __hcc_atomic_is_lock_free(obj)

#define atomic_store(obj, value) \
    __hcc_atomic_store((obj), (value), memory_order_seq_cst)
#define atomic_store_explicit(obj, value, order) \
    __hcc_atomic_store((obj), (value), (order))

#define atomic_load(obj) __hcc_atomic_load((obj), memory_order_seq_cst)
#define atomic_load_explicit(obj, order) __hcc_atomic_load((obj), (order))

#define atomic_exchange(obj, value) \
    __hcc_atomic_exchange((obj), (value), memory_order_seq_cst)
#define atomic_exchange_explicit(obj, value, order) \
    __hcc_atomic_exchange((obj), (value), (order))

#define atomic_compare_exchange_strong(obj, expected, desired) \
    __hcc_atomic_compare_exchange( \
        (obj), (expected), (desired), memory_order_seq_cst, memory_order_seq_cst, 0)
#define atomic_compare_exchange_strong_explicit(obj, expected, desired, succ, fail) \
    __hcc_atomic_compare_exchange((obj), (expected), (desired), (succ), (fail), 0)
#define atomic_compare_exchange_weak(obj, expected, desired) \
    __hcc_atomic_compare_exchange( \
        (obj), (expected), (desired), memory_order_seq_cst, memory_order_seq_cst, 1)
#define atomic_compare_exchange_weak_explicit(obj, expected, desired, succ, fail) \
    __hcc_atomic_compare_exchange((obj), (expected), (desired), (succ), (fail), 1)

#define atomic_fetch_add(obj, operand) \
    __hcc_atomic_fetch_add((obj), (operand), memory_order_seq_cst)
#define atomic_fetch_add_explicit(obj, operand, order) \
    __hcc_atomic_fetch_add((obj), (operand), (order))
#define atomic_fetch_sub(obj, operand) \
    __hcc_atomic_fetch_sub((obj), (operand), memory_order_seq_cst)
#define atomic_fetch_sub_explicit(obj, operand, order) \
    __hcc_atomic_fetch_sub((obj), (operand), (order))
#define atomic_fetch_or(obj, operand) \
    __hcc_atomic_fetch_or((obj), (operand), memory_order_seq_cst)
#define atomic_fetch_or_explicit(obj, operand, order) \
    __hcc_atomic_fetch_or((obj), (operand), (order))
#define atomic_fetch_xor(obj, operand) \
    __hcc_atomic_fetch_xor((obj), (operand), memory_order_seq_cst)
#define atomic_fetch_xor_explicit(obj, operand, order) \
    __hcc_atomic_fetch_xor((obj), (operand), (order))
#define atomic_fetch_and(obj, operand) \
    __hcc_atomic_fetch_and((obj), (operand), memory_order_seq_cst)
#define atomic_fetch_and_explicit(obj, operand, order) \
    __hcc_atomic_fetch_and((obj), (operand), (order))

#define atomic_flag_test_and_set(obj) \
    atomic_flag_test_and_set_explicit((obj), memory_order_seq_cst)
#define atomic_flag_test_and_set_explicit(obj, order) \
    ((_Bool)__hcc_atomic_exchange(&(obj)->__val, 1, (order)))
#define atomic_flag_clear(obj) \
    atomic_flag_clear_explicit((obj), memory_order_seq_cst)
#define atomic_flag_clear_explicit(obj, order) \
    __hcc_atomic_store(&(obj)->__val, 0, (order))

#endif
