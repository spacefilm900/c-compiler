/* Minimised from Csmith seeds 121, 755, 819, 1468, 1841, and 1855 at -O0.
   After a discarded return of a bit-field struct, an int flag of 0 is
   treated as true when passed as the third argument of a function
   whose first two parameters are an integer and a pointer.
   GCC and Clang print nothing (flag is false). This compiler, with
   the IR linked at -O0, prints the flag-true path. The IR linked at
   -O2 agrees with the references. The checksum values in the original
   seeds still match; only the extra transparent_crc traces differ.
*/
#include <stdio.h>
#include <stdint.h>

struct S0 {
    unsigned f0 : 27;
    unsigned f1 : 6;
    unsigned f2 : 17;
    signed f3 : 24;
};

static struct S0 g = {7765, 3, 141, 2755};
static int32_t v = 0xEE1C9741;

static struct S0 func_1(void) { return g; }

static void show(uint64_t val, char *name, int flag)
{
    (void)val;
    if (flag)
        printf("...checksum after hashing %s\n", name);
}

int main(void)
{
    int print_hash_value = 0;
    func_1();
    show(v, "g_4", print_hash_value);
    return 0;
}
