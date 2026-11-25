/* Shared by the conformance probes: a failed CHECK prints a FAIL line. */
#ifndef CONFORMANCE_CHECK_H
#define CONFORMANCE_CHECK_H
#include <stdio.h>
static int check_failures;
#define CHECK(c) ((c) ? (void)0 : (void)(check_failures++, printf("FAIL line %d: %s\n", __LINE__, #c)))
#define DONE() (printf("done %d\n", check_failures), check_failures != 0)
#endif
