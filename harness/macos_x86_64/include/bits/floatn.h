/* Linux-side proxy for Intel macOS (harness/intel_mac_proxy.py only).
 *
 * glibc's own <bits/floatn.h> turns on the _Float128 interfaces for any
 * x86_64 Clang. Clang for x86_64-apple-macos has no __float128, as macOS
 * has none, so this replacement, found first, says the type is absent.
 * long double is the x87 80-bit format on both systems, so _Float64x
 * stays long double. Not used by the compiler or on a Mac.
 */
#ifndef _BITS_FLOATN_H
#define _BITS_FLOATN_H

#include <features.h>

#define __HAVE_FLOAT128 0
#define __HAVE_DISTINCT_FLOAT128 0
#define __HAVE_FLOAT64X 1
#define __HAVE_FLOAT64X_LONG_DOUBLE 1

#include <bits/floatn-common.h>

#endif
