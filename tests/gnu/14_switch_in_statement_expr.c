/* GNU C: a statement expression inside a switch may hold a switch of its
   own, and may leave the enclosing loop or switch with break or continue.
   A case label of the outer switch inside it is an error (GCC manual,
   "Statements and Declarations in Expressions"); see the unit tests. */

int printf(const char *, ...);

static int classify(int x)
{
    int r = -1;
    switch (x) {
    case 0:
        r = ({
            int t = 0;
            switch (x + 1) {
            case 1: t = 10; break;
            default: t = 20; break;
            }
            t;
        });
        break;
    case 1:
        ({ r = 1; if (x) break; r = 99; });
        r += 100;
        break;
    default:
        r = ({ int u = x * 2; u; });
    }
    return r;
}

int main(void)
{
    int sum = 0;
    for (int i = 0; i < 6; i++) {
        int v = ({ if (i == 2) continue; i; });
        sum += v;
    }
    printf("%d %d %d %d\n", classify(0), classify(1), classify(5), sum);
    return 0;
}
