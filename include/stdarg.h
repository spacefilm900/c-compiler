/* Compiler-provided <stdarg.h> for the x86_64 System V ABI.
 *
 * va_list is a one-element array of the register-save area descriptor.
 * As a parameter it decays to a pointer, which is what the library's
 * vprintf expects. stdio.h includes this with __need___va_list set and
 * must not define the name va_list. va_start, va_arg, and va_copy expand
 * to undeclared names: defining a variadic function is not supported yet,
 * and a use of those macros is a diagnostic instead of a null dereference.
 */
#ifndef __CC_VA_LIST
#define __CC_VA_LIST
typedef struct {
    unsigned int gp_offset;
    unsigned int fp_offset;
    void *overflow_arg_area;
    void *reg_save_area;
} __cc_va_elem;
typedef __cc_va_elem __gnuc_va_list[1];
#endif

#ifndef __need___va_list
#ifndef _STDARG_H
#define _STDARG_H
typedef __gnuc_va_list va_list;
#define va_start(ap, last) (__cc_va_start_is_not_supported)
#define va_end(ap) ((void)(ap))
#define va_arg(ap, type) (__cc_va_arg_is_not_supported)
#define va_copy(dest, src) (__cc_va_copy_is_not_supported)
#endif
#endif
#undef __need___va_list
