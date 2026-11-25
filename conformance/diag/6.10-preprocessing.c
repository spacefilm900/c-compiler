/* N1570 6.10 preprocessing directives: constraint violations and #error */
//== 6.10.1p1 #if with no expression
#if
#endif
//== 6.10.1p1 #if with an unbalanced parenthesis
#if (1
#endif
//== 6.10.1p1 #if with an assignment
#if 1 = 1
#endif
//== 6.10.1p1 #if with a comma (not in an unevaluated operand)
#if (1, 2)
#endif
//== 6.10.1p1 a defined operator with no identifier
#if defined
#endif
//== 6.10.1p4 #if dividing by zero
#if 1 / 0
#endif
//== 6.10 #endif without #if
#endif
//== 6.10 #else after #else
#if 1
#else
#else
#endif
//== 6.10 #elif after #else
#if 1
#else
#elif 1
#endif
//== 6.10 an #if without #endif
#if 1
int a;
//== 6.10.2p1 #include of a missing header
#include <no/such/header.h>
//== 6.10.2p4 #include with a bad form
#include no_quotes
//== 6.10.3p2 a non-identical macro redefinition
#define M 1
#define M 2
//== 6.10.3p2 a function-like redefinition with other parameter names
#define F(a) a
#define F(b) b
//== 6.10.3p4 too few macro arguments
#define F(a, b) a
int x = F(1);
//== 6.10.3p4 too many macro arguments
#define F(a) a
int x = F(1, 2);
//== 6.10.3p5 __VA_ARGS__ outside a variadic macro
#define F(a) __VA_ARGS__
//== 6.10.3p6 a duplicate macro parameter
#define F(a, a) a
//== 6.10.3p10 an unterminated macro invocation
#define F(a) a
int x = F(1;
//== 6.10.3.2p1 # not followed by a parameter
#define F(a) # b
//== 6.10.3.3p1 ## at the start of a replacement list
#define F(a) ## a
//== 6.10.3.3p1 ## at the end of a replacement list
#define F(a) a ##
//== 6.10.3.3p3 pasting that does not form a valid token
#define CAT(a, b) a ## b
int x = CAT(+, -) 1;
//== 6.10.3.5p2 defining defined
#define defined 1
//== 6.10.8p2 defining __STDC__
#define __STDC__ 2
//== 6.10.8p2 undefining __LINE__
#undef __LINE__
//== 6.10.4p3 #line with zero
#line 0
//== 6.10.4p3 #line out of range
#line 2147483648
//== 6.10.4p3 #line with a non-number
#line abc
//== 6.10.5 #error
#error this must stop translation
//== 6.10.9 _Pragma with a non-string operand
_Pragma(1)
//== 6.10 an invalid directive name in an active group
#frobnicate
