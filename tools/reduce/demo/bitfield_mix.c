/* Reducer demo input: a mixed program that the compiler at commit 0cab101
   miscompiles. Somewhere in it is a floating compound assignment to a
   bit-field, which overwrote the neighbouring bit-field (pull request 46,
   bug b). See tools/reduce/README.md. */
#include <stdio.h>
#include <string.h>

struct account {
    char name[16];
    long balance;
    int history[8];
    int count;
};

struct packed_flags {
    unsigned level : 4;
    unsigned mode : 4;
    unsigned spare : 8;
};

union number {
    int i;
    float f;
};

static int checksum(const char *text) {
    int sum = 0;
    while (*text) {
        sum = sum * 31 + (unsigned char)*text;
        text++;
    }
    return sum & 0xffff;
}

static void deposit(struct account *acct, int amount) {
    acct->balance += amount;
    if (acct->count < 8)
        acct->history[acct->count++] = amount;
}

static long total_history(const struct account *acct) {
    long total = 0;
    for (int i = 0; i < acct->count; i++)
        total += acct->history[i];
    return total;
}

static int fib(int n) {
    int a = 0, b = 1;
    for (int i = 0; i < n; i++) {
        int t = a + b;
        a = b;
        b = t;
    }
    return a;
}

static double average(const int *values, int n) {
    double sum = 0;
    for (int i = 0; i < n; i++)
        sum += values[i];
    return n ? sum / n : 0.0;
}

static unsigned rotate(unsigned value, int bits) {
    return (value << bits) | (value >> (32 - bits));
}

static int classify(int c) {
    switch (c) {
    case 'a': case 'e': case 'i': case 'o': case 'u':
        return 1;
    case ' ':
        return 2;
    default:
        return c >= '0' && c <= '9' ? 3 : 0;
    }
}

static int adjust_flags(struct packed_flags *flags, double step) {
    flags->level += step;
    return flags->level * 100 + flags->mode * 10 + (flags->spare & 7);
}

static int sum_classes(const char *text) {
    int counts[4] = { 0 };
    for (size_t i = 0; i < strlen(text); i++)
        counts[classify(text[i])]++;
    return counts[0] + 2 * counts[1] + 3 * counts[2] + 4 * counts[3];
}

int main(void) {
    struct account acct;
    memset(&acct, 0, sizeof acct);
    strcpy(acct.name, "demo");
    for (int i = 1; i <= 5; i++)
        deposit(&acct, i * 10);
    printf("balance %ld history %ld\n", acct.balance, total_history(&acct));

    printf("fib %d %d %d\n", fib(10), fib(20), fib(30));

    int values[] = { 3, 1, 4, 1, 5, 9, 2, 6 };
    printf("average %.3f\n", average(values, 8));

    printf("rotate %u\n", rotate(0x12345678u, 8));

    union number num;
    num.f = 1.5f;
    printf("bits %d\n", num.i != 0);

    printf("checksum %d\n", checksum(acct.name));
    printf("classes %d\n", sum_classes("hello world 42"));

    struct packed_flags flags = { 3, 7, 0x55 };
    printf("flags %d\n", adjust_flags(&flags, 1.0));
    printf("mode %u\n", flags.mode);
    return 0;
}
