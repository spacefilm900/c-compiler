/* N1570 6.7 declarations: constraint violations */
//== 6.7p2 a declaration that declares nothing
int;
//== 6.7p3 two declarations of an identifier with no linkage
void h(void) { int a; int a; }
//== 6.7p3 a typedef redefined to a different type
typedef int T; typedef long T;
//== 6.7p3 a tag redefined in the same scope
struct s { int a; }; struct s { int a; };
//== 6.7p4 conflicting types for one object
int a; long a;
//== 6.7p4 conflicting types for one function
int f(int); int f(long);
//== 6.7.1p2 two storage-class specifiers
static extern int a;
//== 6.7.1p3 _Thread_local at block scope without static or extern
void h(void) { _Thread_local int t; }
//== 6.7.1p3 _Thread_local on a function
_Thread_local void h(void);
//== 6.7.1p7 a block-scope function with static
void h(void) { static void inner(void); }
//== 6.9p2 auto at file scope
auto int a;
//== 6.9p2 register at file scope
register int a;
//== 6.7.2p2 no type specifier (implicit int)
x = 1;
//== 6.7.2p2 an invalid type specifier combination
long char c;
//== 6.7.2p2 signed and unsigned together
signed unsigned int x;
//== 6.7.2p2 a typedef name with another specifier
typedef int T; unsigned T x;
//== 6.7.2p2 three longs
long long long x;
//== 6.7.2p3 _Complex not supported (an optional feature, 6.10.8.3)
double _Complex z;
//== 6.7.2.1p2 a member of incomplete type
struct s { struct undefined u; };
//== 6.7.2.1p2 a member of function type
struct s { void f(void); };
//== 6.7.2.1p3 a structure containing itself
struct s { struct s inner; };
//== 6.7.2.1p3 a flexible array member that is not last
struct s { int n; int fam[]; int after; };
//== 6.7.2.1p3 a flexible array member as the only member
struct s { int fam[]; };
//== 6.7.2.1p3 a structure with a flexible array member as an array element
struct s { int n; int fam[]; }; struct s arr[2];
//== 6.7.2.1p3 a duplicate member name
struct s { int a; double a; };
//== 6.7.2.1p3 duplicate names through an anonymous member
struct s { int a; struct { int a; }; };
//== 6.7.2.1p4 a bit-field wider than its type
struct s { int b : 33; };
//== 6.7.2.1p4 a negative bit-field width
struct s { int b : -1; };
//== 6.7.2.1p4 a named zero-width bit-field
struct s { int b : 0; };
//== 6.7.2.1p5 a bit-field of floating type
struct s { double b : 3; };
//== 6.7.2.1p4 a _Bool bit-field wider than one bit
struct s { _Bool b : 2; };
//== 6.7.2.2p2 an enumeration constant outside int
enum e { BIG = 2147483648 };
//== 6.7.2.2 a duplicate enumeration constant
enum e { A, A };
//== 6.7.2.3p1 a tag used with the wrong keyword
struct s { int a; }; union s *p;
//== 6.7.2.3p3 an enum declared without a list before definition
enum e x;
//== 6.7.2.4p3 _Atomic of an array type
_Atomic(int[3]) a;
//== 6.7.2.4p3 _Atomic of a function type
_Atomic(int (void)) f;
//== 6.7.2.4p3 _Atomic of a qualified type
_Atomic(const int) a;
//== 6.7.3p2 restrict on a non-pointer
restrict int a;
//== 6.7.3p2 restrict on a pointer to function
void (*restrict fp)(void);
//== 6.7.3p3 _Atomic qualifier on an array
typedef int A[3]; _Atomic A a;
//== 6.7.4p3 inline on an object
inline int a;
//== 6.7.4p2 _Noreturn on an object
_Noreturn int a;
//== 6.7.4p3 an inline external definition referring to a static object
static int s; inline int h(void) { return s; }
//== 6.7.4p3 an inline external definition defining a modifiable static
inline void h(void) { static int c; (void)c; }
//== 6.7.4p4 inline main
inline int main(void) { return 0; }
//== 6.7.5p2 _Alignas on a typedef
typedef _Alignas(8) int T;
//== 6.7.5p2 _Alignas on a bit-field
struct s { _Alignas(8) int b : 3; };
//== 6.7.5p2 _Alignas on a function
_Alignas(8) void h(void);
//== 6.7.5p2 _Alignas on a parameter
void h(_Alignas(8) int p);
//== 6.7.5p2 _Alignas on a register object
void h(void) { register _Alignas(8) int r; }
//== 6.7.5p3 an alignment that is not a power of two
_Alignas(3) int a;
//== 6.7.5p4 an alignment less strict than the type requires
_Alignas(1) int a;
//== 6.7.6.2p1 an array of functions
void a[3](void);
//== 6.7.6.2p1 an array of zero size
int a[0];
//== 6.7.6.2p1 an array of negative size
int a[-1];
//== 6.7.6.2p1 an array of incomplete element type
struct undefined a[3];
//== 6.7.6.2p1 static in a non-parameter array
int a[static 3];
//== 6.7.6.2p1 static in a non-outermost parameter array dimension
void h(int a[3][static 3]);
//== 6.7.6.2p2 a variably modified type at file scope
int n = 3; int (*p)[n];
//== 6.7.6.2p2 a VLA with static storage
void h(int n) { static int v[n]; }
//== 6.7.6.2p2 a VLA member
void h(int n) { struct s { int v[n]; }; }
//== 6.7.6.2p2 an extern VLA
void h(int n) { extern int v[n]; }
//== 6.7.6.2p4 [*] outside a prototype
void h(void) { int v[*]; }
//== 6.7.6.3p1 a function returning an array
int h(void)[3];
//== 6.7.6.3p1 a function returning a function
int h(void)(void);
//== 6.7.6.3p2 a storage class other than register on a parameter
void h(static int p);
//== 6.7.6.3p3 an identifier list in a declaration that is not a definition
int h(a, b);
//== 6.7.6.3p4 an incomplete parameter type in a definition
struct undefined; void h(struct undefined p) { }
//== 6.7.6.3p10 void with a name
void h(void x);
//== 6.7.6.3p10 void with other parameters
void h(void, int);
//== 6.7.6.3p14 ... with no named parameter
void h(...);
//== 6.7.9p2 too many initializers for an array
int a[2] = { 1, 2, 3 };
//== 6.7.9p2 too many initializers for a scalar
int a = { 1, 2 };
//== 6.7.9p2 too many initializers for a structure
struct s { int a; } v = { 1, 2 };
//== 6.7.9p3 initializing a variable length array
void h(int n) { int v[n] = { 0 }; }
//== 6.7.9p3 initializing a function
void h(void) = 0;
//== 6.7.9p4 a non-constant initializer for a static object
int g; int *p = (int *)g;
//== 6.7.9p5 an initializer on a block-scope extern
void h(void) { extern int e = 1; }
//== 6.7.9p6 an array designator in a structure initializer
struct s { int a; } v = { [0] = 1 };
//== 6.7.9p6 an array designator out of range
int a[2] = { [2] = 1 };
//== 6.7.9p6 a negative array designator
int a[2] = { [-1] = 1 };
//== 6.7.9p7 a member designator in an array initializer
int a[2] = { .x = 1 };
//== 6.7.9p7 a designator naming a missing member
struct s { int a; } v = { .b = 1 };
//== 6.7.9p11 a scalar initialised with an incompatible type
struct s { int a; } v; int a = v;
//== 6.7.9p14 a string literal too long for its array
char s[2] = "abc";
//== 6.7.9p14 a wide string for a char array
char s[] = L"abc";
//== 6.7.10p2 a failing static assertion
_Static_assert(0, "fails");
//== 6.7.10p2 a non-constant static assertion
int g; _Static_assert(g, "not constant");
