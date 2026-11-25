/* N1570 6.4.6 punctuators (digraphs), 6.4.7 header names, 6.4.8 pp-numbers, 6.4.9 comments */
%:include "check.h"
%:define STR(x) %:x
%:define CAT(a, b) a %:%: b

int main(void) <%
    int a<:3:> = <%1, 2, 3%>;
    CHECK(a<:2:> == 3);
    CHECK(sizeof STR(<:) == 3);           /* a digraph is spelled as written when stringised */
    int CAT(x, y) = 4;
    CHECK(xy == 4);
    int i = 1;
    i = i+++i;                             /* 6.4p4: longest match: (i++) + i */
    (void)i;
    int j = 2, k = 3;
    CHECK(j+++k == 5 && j == 3);
    CHECK(5 // comment
          / 5 == 1);
    CHECK(7 /* a /* does not nest */ == 7);
    // a line comment may contain /* without closing it
    CHECK(sizeof STR(0x1e+1) == sizeof "0x1e+1");   /* 6.4.8: one pp-number */
    CHECK(sizeof STR(1.2.3.4) == 8);
    CHECK(sizeof STR(.5e-x) == 6);
    int ar = 1 ? 2 : 3, *p = &ar;
    return DONE();
%>
