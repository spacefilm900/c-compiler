/* Maximal munch of punctuators (C11 6.4p4, 6.4.6): the longest sequence of
   characters that forms a punctuator is one token, so `a+++b` is
   `a++ + b`, `x<<=1` is one compound assignment, `..` is two tokens and
   `...` is one. Digraphs (6.4.6p3) `<: :> <% %>` are the same tokens as
   `[ ] { }`. This file has no preprocessor directives, as required of
   tests/ from 119 onward. */

int printf(const char *, ...);

struct pt { int x, y; };

static int sum(int n, ...)
{
    return n;
}

static int pick(int a<:3:>, int i) <%
    return a<:i:>;
%>

int main(void)
{
    int a = 5, b = 2, c;
    int arr<:4:> = <% 1, 2, 3, 4 %>;
    struct pt p = <% 7, 9 %>;
    struct pt *q = &p;
    unsigned u = 0xF0u;
    int shifted;

    c = a+++b;            /* a++ + b */
    printf("%d %d\n", c, a);
    c = a---b;            /* a-- - b */
    printf("%d %d\n", c, a);
    c = a - -b;
    printf("%d\n", c);
    c = a+ +b;
    printf("%d\n", c);

    shifted = 1;
    shifted <<= 3;        /* <<= is one token */
    printf("%d\n", shifted);
    shifted >>= 1;
    printf("%d\n", shifted);
    shifted = 16 >> 2 << 1;
    printf("%d\n", shifted);

    u &= 0x3Cu;
    u |= 1u;
    u ^= 0x0Fu;
    printf("%u\n", u);
    printf("%d %d\n", a <= b, a >= b);
    printf("%d %d\n", a == b, a != b);
    printf("%d %d\n", a && b, a || !b);

    printf("%d %d %d\n", q->x, q->y, (*q).x);
    printf("%d\n", pick(arr, 2));
    printf("%d\n", sum(3, 1, 2, 3));
    printf("%d\n", a%b);
    a %= 3;
    printf("%d\n", a);
    a *= 4; a /= 2; a += 1; a -= 2;
    printf("%d\n", a);
    printf("%d\n", a ? b : c);
    printf("%d\n", ~a);
    return 0;
}
