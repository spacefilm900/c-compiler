/* N1570 6.8 statements: constraint violations */
//== 6.8.1p2 case outside a switch
void h(void) { case 1: ; }
//== 6.8.1p2 default outside a switch
void h(void) { default: ; }
//== 6.8.1p3 a duplicate label
void h(void) { a: ; a: ; }
//== 6.8.4.2p2 a jump into the scope of a variably modified type
void h(int n, int v) { switch (v) { int a[n]; case 1: a[0] = 1; } }
//== 6.8.4.2p3 duplicate case values
void h(int v) { switch (v) { case 1: case 1: ; } }
//== 6.8.4.2p3 duplicate case values after conversion
void h(unsigned int v) { switch (v) { case -1: case 0xffffffffu: ; } }
//== 6.8.4.2p3 two default labels
void h(int v) { switch (v) { default: ; default: ; } }
//== 6.8.4.2p1 a switch on a floating value
void h(double d) { switch (d) { case 1: ; } }
//== 6.8.4.2p1 a switch on a pointer
void h(int *p) { switch (p) { default: ; } }
//== 6.8.4p1 if on a structure
struct s { int a; }; void h(struct s v) { if (v) ; }
//== 6.8.5p2 while on a structure
struct s { int a; }; void h(struct s v) { while (v) ; }
//== 6.8.5p3 a for declaration with static
void h(void) { for (static int i = 0; i < 1; ) ; }
//== 6.8.5p3 a for declaration of a function
void h(void) { for (int g(void); ; ) break; }
//== 6.8.6.1p1 goto an undefined label
void h(void) { goto missing; }
//== 6.8.6.1p1 goto into the scope of a variable length array
void h(int n) { goto in; { int a[n]; in: a[0] = 0; } }
//== 6.8.6.2p1 continue outside a loop
void h(void) { continue; }
//== 6.8.6.2p1 continue in a switch outside a loop
void h(int v) { switch (v) { case 1: continue; } }
//== 6.8.6.3p1 break outside a loop or switch
void h(void) { break; }
//== 6.8.6.4p1 return with a value in a void function
void h(void) { return 1; }
//== 6.8.6.4p1 return without a value in a non-void function
int h(void) { return; }
//== 6.8.6.4p3 return of an incompatible type
struct s { int a; } sv; int h(void) { return sv; }
//== 6.8.1 a label at the end of a compound statement (C11 needs a statement)
void h(void) { end: }
//== 6.8.1 a declaration after a label (C11 needs a statement)
void h(void) { lab: int x = 1; (void)x; }
