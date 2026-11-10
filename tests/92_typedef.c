typedef int T;
typedef T *TP;
typedef struct {
    int x;
    int y;
} Pair;
typedef int Func(int);
typedef Func *FuncPtr;

int id(int n) { return n; }

int main(void) {
    Pair p;
    TP xp;
    FuncPtr fp;
    int Tshadow;
    p.x = 3;
    p.y = 4;
    xp = &p.x;
    fp = id;
    Tshadow = 5;
    {
        typedef int T;
        T inner = 6;
        if (inner != 6) {
            return 1;
        }
    }
    return *xp + p.y + fp(10) + Tshadow;
}
