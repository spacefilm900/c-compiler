/*
 * A deterministic memory allocator for the DOOM differential harness.
 * This file is part of this repository (MIT licence).
 *
 * DOOM draws a few frames from memory it never initialised: the first
 * screen wipe starts from a video buffer that was never cleared, and a
 * handful of later frames have stray pixels read from outside the data
 * they draw. That memory holds old heap contents, including addresses.
 * With the C library's allocator those addresses depend on address-space
 * randomisation and on the size of the executable, so the same frames
 * differ from run to run and between a Clang build and this compiler's
 * build even when both are correct.
 *
 * This file replaces malloc and its relatives for the whole program,
 * including the C library's own use of them, with a bump allocator in
 * one region mapped at a fixed address. Every allocation then lands at
 * the same address in every run and every build. free() does nothing:
 * the demo run allocates only a few megabytes in total.
 *
 * On macOS the C library keeps its own allocator for its internal use
 * (symbols bind per library there), but every call DOOM makes comes
 * here, which is what makes the frames repeatable. macOS has no
 * MAP_FIXED_NOREPLACE, so the address is passed as a hint and the result
 * checked: the arena is never mapped over anything already there.
 */

#define _GNU_SOURCE
#include <stddef.h>
#include <stdint.h>
#include <string.h>
#include <sys/mman.h>
#include <unistd.h>

#ifndef MAP_NORESERVE
#define MAP_NORESERVE 0
#endif
#ifdef MAP_FIXED_NOREPLACE
#define ARENA_FIXED MAP_FIXED_NOREPLACE
#else
#define ARENA_FIXED 0
#endif

#define ARENA_ADDR ((uintptr_t)0x200000000000u)
#define ARENA_SIZE ((size_t)1 << 30)
#define HEADER 16u

static unsigned char *arena_top;
static unsigned char *arena_end;

static void die(const char *msg, size_t len)
{
    ssize_t ignored = write(2, msg, len);
    (void)ignored;
    _exit(3);
}

static void arena_init(void)
{
    void *p = mmap((void *)ARENA_ADDR, ARENA_SIZE, PROT_READ | PROT_WRITE,
                   MAP_PRIVATE | MAP_ANONYMOUS | MAP_NORESERVE | ARENA_FIXED, -1, 0);
    if (p != (void *)ARENA_ADDR)
        die("dg_alloc: cannot map the fixed arena\n", 37);
    arena_top = p;
    arena_end = arena_top + ARENA_SIZE;
}

/* Each block is preceded by its requested size, for realloc. */
void *memalign(size_t align, size_t size)
{
    uintptr_t at;

    if (arena_top == NULL)
        arena_init();
    if (align < HEADER)
        align = HEADER;
    if ((align & (align - 1)) != 0)
        return NULL;
    at = ((uintptr_t)arena_top + HEADER + align - 1) & ~(uintptr_t)(align - 1);
    if (size > (size_t)(arena_end - (unsigned char *)at))
        die("dg_alloc: arena exhausted\n", 26);
    ((size_t *)at)[-1] = size;
    arena_top = (unsigned char *)(at + size);
    return (void *)at;
}

void *malloc(size_t size)
{
    return memalign(HEADER, size);
}

void free(void *ptr)
{
    (void)ptr;
}

void *calloc(size_t count, size_t size)
{
    void *p;

    if (size != 0 && count > (size_t)-1 / size)
        return NULL;
    p = malloc(count * size);
    memset(p, 0, count * size);
    return p;
}

void *realloc(void *ptr, size_t size)
{
    void *q = malloc(size);
    if (ptr != NULL) {
        size_t old = ((size_t *)ptr)[-1];
        memcpy(q, ptr, old < size ? old : size);
    }
    return q;
}

int posix_memalign(void **out, size_t align, size_t size)
{
    void *p = memalign(align, size);
    if (p == NULL)
        return 22; /* EINVAL */
    *out = p;
    return 0;
}

void *aligned_alloc(size_t align, size_t size)
{
    return memalign(align, size);
}

size_t malloc_usable_size(void *ptr)
{
    return ptr == NULL ? 0 : ((size_t *)ptr)[-1];
}
