#include "shared.h"
int vari(int n,...){va_list ap;va_start(ap,n);struct S s=va_arg(ap,struct S);int k=va_arg(ap,int);va_end(ap);return n+s.a+s.x+s.b+k;}
