/* N1570 6.9 external definitions: constraint violations */
//== 6.9p3 two definitions of one function
int h(void) { return 0; } int h(void) { return 1; }
//== 6.9p3 two definitions of one object
int a = 1; int a = 2;
//== 6.9p5 an undefined static function that is used
static int h(void); int use(void) { return h(); }
//== 6.9.1p2 a function definition whose declarator has no parameter list
typedef int F(void); F h { return 0; }
//== 6.9.1p3 a definition returning an array (6.7.6.3p1)
int h(void)[3] { }
//== 6.9.1p5 an unnamed parameter in a definition
int h(int) { return 0; }
//== 6.9.1p6 an identifier-list parameter declared twice
int h(a) int a; int a; { return a; }
//== 6.9.1p6 a declared parameter not in the identifier list
int h(a) int a, b; { return a; }
//== 6.9.1p6 an initializer on an identifier-list parameter
int h(a) int a = 1; { return a; }
//== 6.9.2p3 a static tentative definition of incomplete type
static int a[];
//== 6.9.1p2 a definition with a storage class other than extern or static
register int h(void) { return 0; }
//== 6.7.4p3 main as inline (6.7.4p4)
inline int main(void) { return 0; }
