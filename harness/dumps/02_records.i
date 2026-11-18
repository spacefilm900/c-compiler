
struct point {
char tag;
int x, y;
};

struct flags {
unsigned ready : 1;
unsigned mode : 3;
signed level : 4;
unsigned char extra;
};

union word {
unsigned int value;
unsigned char bytes[4];
};

struct point origin = { 'o', 0, 0 };
struct point *where = &origin;
int table[4] = { 1, 2, [3] = 4 };
int *third = &table[2];
const char *name = "point";

int area(const struct point *p, struct flags f) {
union word w;
w.value = 0;
w.bytes[1] = f.extra;
f.mode += 2;
return p->x * p->y + f.mode + f.level + (int)w.value;
}

int main(void) {
struct flags f = { 1, 2, -3, 4 };
struct point p = { .x = 3, .y = 5 };
return area(&p, f) - 1043 + table[3] - *third - (where == &origin) - (name[0] - 'p');
}
