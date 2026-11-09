/* Minimal stand-in for <stdio.h>.
 *
 * This is not the system header. The compiler does not search the host
 * libc headers yet: typedef, enum, and the rest of the C library are still
 * outside the language subset. Declarations here use only types the compiler
 * already accepts. `unsigned long` stands in for size_t.
 */
#ifndef _CC_STDIO_H
#define _CC_STDIO_H

int printf(const char *, ...);
int puts(const char *);
int putchar(int);

#endif
