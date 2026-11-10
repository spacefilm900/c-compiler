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

#if defined(__CC_STDDEF_H)
#define offsetof(type, member) ((size_t)((char *)&((type *)0)->member - (char *)0))
#endif

#endif
