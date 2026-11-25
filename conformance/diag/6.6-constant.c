/* N1570 6.6 constant expressions */
int g;
int arr[2];
//== 6.6p3 an assignment in an evaluated constant expression
int x = (g = 1);
//== 6.6p3 a comma operator in an evaluated constant expression
int x = (1, 2);
//== 6.6p3 a function call in a constant expression
int f(void); int x = f();
//== 6.6p4 a constant expression outside its type's range
int x = 2147483647 + 1;
//== 6.6p4 a constant division by zero
int x = 1 / 0;
//== 6.6p6 a non-constant array size at file scope
int n = 3; int a[n];
//== 6.6p6 a floating operand not under a cast in an integer constant expression
int a[(int)(1.5 + 1)];
//== 6.6p6 a non-constant case label
void h(int v) { switch (v) { case g: break; } }
//== 6.6p6 a non-integer case label
void h(int v) { switch (v) { case 1.5: break; } }
//== 6.6p6 a non-constant enumeration value
enum { E = g };
//== 6.6p6 a non-constant bit-field width
struct s { int b : g; };
//== 6.6p7 a non-constant static initializer
int y = g;
//== 6.6p9 the value of an object in an address constant
int *p = &arr[g];
//== 6.6p7 a block-scope static initialised from an automatic
void h(void) { int a = 1; static int b = a; (void)b; }
