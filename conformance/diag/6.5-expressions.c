/* N1570 6.5 expressions: constraint violations */
struct s { int a; } sv;
union u { int i; } uv;
int arr[3], *ip, i;
double d;
const int ci = 1;
void vf(void);
int f2(int, int);
//== 6.5.1.1 no _Generic association matches
int x = _Generic(1.0, int: 1);
//== 6.5.1.1 two compatible _Generic associations
int x = _Generic(1, int: 1, signed int: 2);
//== 6.5.1.1 two default associations
int x = _Generic(1, default: 1, default: 2);
//== 6.5.1.1 a variably modified type in an association
void g(int n) { (void)_Generic(1, int[n]: 1, default: 0); }
//== 6.5.2.1 subscripting a non-pointer
void g(void) { (void)i[0]; }
//== 6.5.2.2 too few arguments to a prototyped function
void g(void) { f2(1); }
//== 6.5.2.2 too many arguments to a prototyped function
void g(void) { f2(1, 2, 3); }
//== 6.5.2.2 an argument not assignable to the parameter type
void g(void) { f2(sv, 1); }
//== 6.5.2.2 calling a non-function
void g(void) { i(); }
//== 6.5.2.3 a member that does not exist
void g(void) { sv.b = 1; }
//== 6.5.2.3 . on a pointer
void g(void) { struct s *p = &sv; p.a = 1; }
//== 6.5.2.3 -> on a structure
void g(void) { sv->a = 1; }
//== 6.5.2.4 ++ on a const object
void g(void) { ci++; }
//== 6.5.2.4 ++ on a non-lvalue
void g(void) { (i + 1)++; }
//== 6.5.2.5 a compound literal of a variable length array type
void g(int n) { (void)(int[n]){ 1 }; }
//== 6.5.3.1 -- on a structure
void g(void) { --sv; }
//== 6.5.3.2 & of a register object
void g(void) { register int r = 0; int *p = &r; (void)p; }
//== 6.5.3.2 & of a bit-field
struct bf { int b : 3; } bfv; int *p = &bfv.b;
//== 6.5.3.2 & of a non-lvalue
void g(void) { int *p = &(i + 1); (void)p; }
//== 6.5.3.2 * of a non-pointer
void g(void) { (void)*i; }
//== 6.5.3.3 unary minus on a pointer
void g(void) { (void)-ip; }
//== 6.5.3.3 ~ on a double
void g(void) { (void)~d; }
//== 6.5.3.3 ! on a structure
void g(void) { (void)!sv; }
//== 6.5.3.4 sizeof a function type
unsigned long n = sizeof(void (void));
//== 6.5.3.4 sizeof an incomplete type
struct undefined; unsigned long n = sizeof(struct undefined);
//== 6.5.3.4 sizeof a bit-field
struct bf { int b : 3; } bfv; unsigned long n = sizeof bfv.b;
//== 6.5.3.4 _Alignof a function type
unsigned long n = _Alignof(void (void));
//== 6.5.3.4 _Alignof an incomplete type
unsigned long n = _Alignof(int[]);
//== 6.5.3.4 _Alignof of an expression (not a type name)
unsigned long n = _Alignof(i);
//== 6.5.4 a cast to a structure type
void g(void) { (void)(struct s)sv; }
//== 6.5.4 a cast from a structure
void g(void) { (void)(int)sv; }
//== 6.5.4 a cast between a pointer and a floating type
void g(void) { (void)(double)ip; }
//== 6.5.5 % with a floating operand
void g(void) { (void)(d % 2); }
//== 6.5.5 * with a pointer operand
void g(void) { (void)(ip * 2); }
//== 6.5.6 adding two pointers
void g(void) { (void)(ip + ip); }
//== 6.5.6 subtracting pointers to incompatible types
void g(void) { double *dp = &d; (void)(ip - dp); }
//== 6.5.6 pointer arithmetic on a function pointer
void g(void) { void (*fp)(void) = vf; (void)(fp + 1); }
//== 6.5.6 pointer arithmetic on void *
void g(void) { void *vp = 0; (void)(vp + 1); }
//== 6.5.7 a shift of a floating value
void g(void) { (void)(d << 1); }
//== 6.5.8 relational comparison of a pointer and an integer
void g(void) { (void)(ip < 1); }
//== 6.5.8 relational comparison of incompatible pointers
void g(void) { double *dp = &d; (void)(ip < dp); }
//== 6.5.9 equality of a pointer and a non-zero integer
void g(void) { (void)(ip == 1); }
//== 6.5.9 equality of incompatible pointers
void g(void) { double *dp = &d; (void)(ip == dp); }
//== 6.5.9 equality of structures
void g(void) { (void)(sv == sv); }
//== 6.5.10 & with a floating operand
void g(void) { (void)(d & 1); }
//== 6.5.13 && with a structure operand
void g(void) { (void)(sv && 1); }
//== 6.5.15 a conditional whose first operand is a structure
void g(void) { (void)(sv ? 1 : 2); }
//== 6.5.15 a conditional with a structure and an integer
void g(void) { (void)(i ? sv : 1); }
//== 6.5.15 a conditional with incompatible pointers
void g(void) { double *dp = &d; (void)(i ? ip : dp); }
//== 6.5.16 assignment to a const object
void g(void) { ci = 2; }
//== 6.5.16 assignment to an array
void g(void) { int b[3]; arr = b; }
//== 6.5.16 assignment to a non-lvalue
void g(void) { i + 1 = 2; }
//== 6.5.16 assignment to a structure with a const member
struct cm { const int c; } c1, c2; void g(void) { c1 = c2; }
//== 6.5.16.1 an integer to a pointer
void g(void) { ip = 5; }
//== 6.5.16.1 a pointer to an integer
void g(void) { i = ip; }
//== 6.5.16.1 incompatible pointer types
void g(void) { double *dp = 0; ip = dp; }
//== 6.5.16.1 discarding a qualifier
void g(void) { const int *cp = 0; ip = cp; }
//== 6.5.16.1 a structure to a different structure
void g(void) { sv = uv; }
//== 6.5.16.1 a function pointer to an object pointer
void g(void) { void *vp = vf; (void)vp; }
//== 6.5.16.2 += on a pointer with a pointer
void g(void) { ip += ip; }
//== 6.5.16.2 %= on a double
void g(void) { d %= 2; }
