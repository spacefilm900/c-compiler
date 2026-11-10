void qsort(void *, unsigned long, unsigned long, int (*)(const void *, const void *));

typedef int bin(int, int);
typedef bin *binptr;

int add(int a, int b) { return a + b; }
int sub(int a, int b) { return a - b; }

int apply(binptr fn, int a, int b) { return fn(a, b); }
int apply_call(int (*fn)(int, int), int a, int b) { return (*fn)(a, b); }

struct ops {
    int (*fn)(int, int);
};

binptr choose(int which) { return which ? add : sub; }

int cmp(const void *a, const void *b) {
    int x = *(const int *)a;
    int y = *(const int *)b;
    return (x > y) - (x < y);
}

int main(void) {
    int (*p)(int, int) = add;
    int (*q)(int, int) = &add;
    if (p(2, 3) != 5 || (*q)(4, 6) != 10) {
        return 1;
    }
    if (p != q || p == 0) {
        return 2;
    }
    if (apply(add, 20, 22) != 42 || apply_call(sub, 10, 3) != 7) {
        return 3;
    }
    int (*table[2])(int, int) = {add, sub};
    if (table[0](8, 1) != 9 || table[1](8, 1) != 7) {
        return 4;
    }
    struct ops op = {add};
    if (op.fn(6, 7) != 13) {
        return 5;
    }
    binptr r = choose(1);
    if (r(3, 4) != 7 || choose(0)(9, 2) != 7) {
        return 6;
    }
    int nums[4] = {3, 1, 4, 2};
    qsort(nums, 4, sizeof(int), cmp);
    if (nums[0] != 1 || nums[1] != 2 || nums[2] != 3 || nums[3] != 4) {
        return 7;
    }
    return 0;
}
