/* Compiler-provided <stddef.h> for x86_64 System V (LP64).
 *
 * glibc includes this more than once, each time asking for one name via
 * __need_size_t, __need_ptrdiff_t, __need_wchar_t, or __need_NULL. A normal
 * include defines all of them. The types come from the predefined macros
 * __SIZE_TYPE__, __PTRDIFF_TYPE__, and __WCHAR_TYPE__.
 */
#if !defined(__CC_STDDEF_H) || defined(__need_ptrdiff_t) || defined(__need_size_t) \
    || defined(__need_wchar_t) || defined(__need_NULL)

#if !defined(__need_ptrdiff_t) && !defined(__need_size_t) && !defined(__need_wchar_t) \
    && !defined(__need_NULL)
#define __CC_STDDEF_H
#endif

#if defined(__CC_STDDEF_H) || defined(__need_ptrdiff_t)
#ifndef __CC_PTRDIFF_T
#define __CC_PTRDIFF_T
typedef __PTRDIFF_TYPE__ ptrdiff_t;
#endif
#undef __need_ptrdiff_t
#endif

#if defined(__CC_STDDEF_H) || defined(__need_size_t)
#ifndef __CC_SIZE_T
#define __CC_SIZE_T
typedef __SIZE_TYPE__ size_t;
#endif
#undef __need_size_t
#endif

#if defined(__CC_STDDEF_H) || defined(__need_wchar_t)
#ifndef __CC_WCHAR_T
#define __CC_WCHAR_T
typedef __WCHAR_TYPE__ wchar_t;
#endif
#undef __need_wchar_t
#endif

#if defined(__CC_STDDEF_H) || defined(__need_NULL)
#ifndef NULL
#define NULL ((void *)0)
#endif
#undef __need_NULL
#endif

#if defined(__CC_STDDEF_H) && !defined(__CC_MAX_ALIGN_T)
#define __CC_MAX_ALIGN_T
/* C11 7.19p2: an object type whose alignment is the greatest fundamental
 * alignment, 16 on x86_64 System V (long double). */
typedef struct {
    long long __max_align_ll;
    long double __max_align_ld;
} max_align_t;
#endif

#if defined(__CC_STDDEF_H)
/* C11 7.19p3: an integer constant expression of type size_t. */
/* A program may have defined its own offsetof first (an old idiom); the
   header's definition replaces it rather than clashing with it. */
#undef offsetof
#define offsetof(type, member) __builtin_offsetof(type, member)
#endif

#endif
