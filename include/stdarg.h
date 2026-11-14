/* Compiler-provided <stdarg.h>.
 *
 * On x86_64 System V, __builtin_va_list is the one-element array of the
 * register-save descriptor { gp_offset, fp_offset, overflow_arg_area,
 * reg_save_area }; as a parameter it decays to a pointer, which is what
 * vprintf expects. On Apple arm64 it is `char *`, and on AArch64 Linux it
 * is the AAPCS64 structure { __stack, __gr_top, __vr_top, __gr_offs,
 * __vr_offs }, passed by value.
 * stdio.h includes this with __need___va_list set and must not define
 * the name va_list. The macros are the same builtins the host <stdarg.h>
 * expands to.
 */
#ifndef __CC_VA_LIST
#define __CC_VA_LIST
typedef __builtin_va_list __gnuc_va_list;
#endif

#ifndef __need___va_list
#ifndef _STDARG_H
#define _STDARG_H
typedef __gnuc_va_list va_list;
#define va_start(ap, last) __builtin_va_start(ap, last)
#define va_end(ap) __builtin_va_end(ap)
#define va_arg(ap, type) __builtin_va_arg(ap, type)
#define va_copy(dest, src) __builtin_va_copy(dest, src)
#endif
#endif
#undef __need___va_list
