/* Minimal stand-in for <string.h>. See include/stdio.h. */
#ifndef _CC_STRING_H
#define _CC_STRING_H

unsigned long strlen(const char *);
int strcmp(const char *, const char *);
void *memcpy(void *, const void *, unsigned long);

#endif
