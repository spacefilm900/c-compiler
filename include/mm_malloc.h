/* Compiler-provided <mm_malloc.h>: _mm_malloc and _mm_free, the aligned
 * allocation pair that the x86 SIMD headers name (GCC and Clang provide
 * this header with their own builtin headers). MinGW-w64's <malloc.h>
 * includes it on x86-64. On Windows the C runtime's _aligned_malloc does
 * the work; elsewhere POSIX posix_memalign does.
 */
#ifndef __CC_MM_MALLOC_H
#define __CC_MM_MALLOC_H

#include <stddef.h>

#ifdef _WIN32
void *_aligned_malloc(size_t __size, size_t __align);
void _aligned_free(void *__ptr);

static __inline__ void *_mm_malloc(size_t __size, size_t __align)
{
    return _aligned_malloc(__size, __align);
}

static __inline__ void _mm_free(void *__ptr)
{
    _aligned_free(__ptr);
}
#else
int posix_memalign(void **__ptr, size_t __align, size_t __size);
void free(void *__ptr);

static __inline__ void *_mm_malloc(size_t __size, size_t __align)
{
    void *__ptr;
    if (__align < sizeof(void *))
        __align = sizeof(void *);
    if (posix_memalign(&__ptr, __align, __size) != 0)
        return NULL;
    return __ptr;
}

static __inline__ void _mm_free(void *__ptr)
{
    free(__ptr);
}
#endif

#endif
