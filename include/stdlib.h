/* Minimal stand-in for <stdlib.h>. See include/stdio.h. */
#ifndef _CC_STDLIB_H
#define _CC_STDLIB_H

void *malloc(unsigned long);
void free(void *);
void exit(int);
int abs(int);

#endif
