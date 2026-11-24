/* `__asm__` statements without operands (GCC manual, "Basic Asm" and
   "Extended Asm"), which strict C11 also accepts as a GNU keyword. An
   empty template with a "memory" clobber is a compiler barrier; QuickJS's
   spin wait uses `asm volatile("pause" ::: "memory")` on x86-64. */
int printf(const char *, ...);

static int counter;

static void barrier(void) { __asm__ __volatile__("" ::: "memory"); }

int main(void) {
    __asm__("nop");
    __asm__ volatile("nop" :::);
    __asm__ __volatile__("nop\n\tnop" : : : "memory", "cc");
    for (int i = 0; i < 5; i++) {
        counter += i;
        barrier();
    }
    printf("%d\n", counter);
    return 0;
}
