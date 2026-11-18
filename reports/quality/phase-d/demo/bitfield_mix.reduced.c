#include <stdio.h>
struct packed_flags {
    unsigned level : 4;
    unsigned mode : 4;
    unsigned spare ;
};
int adjust_flags(struct packed_flags *flags, double step) {
    flags->level += step;
    return flags-> mode * 10 + (flags->spare );
}
int main( ) {
    struct packed_flags flags = { 7, 0x55 };
    printf("flags %d\n", adjust_flags(&flags, 1.0));
}
