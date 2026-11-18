


enum colour { RED, GREEN = 5, BLUE };

typedef unsigned long size_type;

static const char *label = "hello";

int pick(int c) {
switch (c) {
case RED:
return 'r';
case GREEN:
case BLUE:
return ((c) * (c));
default:
return _Generic(c, int: 1, default: 2);
}
}

int main(void) {
size_type n = sizeof(long double) + sizeof label;
long double ld = 1.25L;
unsigned long long big = 18446744073709551615ULL;
int var1 = 1 + L'x' - 'x';
return pick(BLUE) - 36 + (int)(n - 24) + (ld < 1) + (int)(big >> 63) - var1 + (int)0x10u - 16;
}
