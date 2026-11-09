char gc = -1;
unsigned char guc = 200;
short gs = -300;
unsigned short gus = 40000;
int gi = -2;
unsigned gu = 1u << 31;
long gl = -1L;
unsigned long gul = 1ul << 40;
long long gll = 1LL << 40;
unsigned long long gull = 1ull << 63;
static unsigned char hidden = 7;
extern short es;
short es = -4;
unsigned char tent;

int main(void) {
    if (gc != -1)
        return 1;
    if (guc != 200)
        return 2;
    if (gs != -300)
        return 3;
    if (gus != 40000)
        return 4;
    if (gi != -2)
        return 5;
    if (gu != 2147483648u)
        return 6;
    if (gl != -1L)
        return 7;
    if ((gul >> 40) != 1ul)
        return 8;
    if ((gll >> 40) != 1LL)
        return 9;
    if ((gull >> 63) != 1ull)
        return 10;
    if (hidden != 7)
        return 11;
    gc = 5;
    if (gc != 5)
        return 12;
    if (tent != 0)
        return 13;
    if (es != -4)
        return 14;
    return 0;
}
