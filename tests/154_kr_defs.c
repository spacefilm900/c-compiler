/* K&R function definitions (C11 6.9.1p6): identifier lists, register
   parameters, and default argument promotions at the ABI boundary. */

int printf(const char *, ...);

int add(a, b)
    int a;
    int b;
{
    return a + b;
}

int widen(c)
    char c;
{
    return c;
}

float narrow(x)
    float x;
{
    return x;
}

int inc(x)
    register int x;
{
    return x + 1;
}

int proto(double);
int proto(x)
    float x;
{
    return (int)(x + 0.5);
}

int main(void) {
    char c = 65;
    float f = 2.5f;
    printf("%d %d %d %d %d\n", add(20, 22), widen(c), inc(41), proto(2.2),
           narrow(f) > 2.4f && narrow(f) < 2.6f);
    return 0;
}
